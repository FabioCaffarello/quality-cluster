package main

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	adapterkafka "internal/adapters/kafka"
	adapternats "internal/adapters/nats"
	dataplaneapp "internal/application/dataplane"
	runtimebootstrap "internal/application/runtimebootstrap"
	configdomain "internal/domain/configctl"
	"internal/shared/bootstrap"
	"internal/shared/problem"
	"internal/shared/settings"
)

var ensureTopicsForBootstrap = adapterkafka.EnsureTopics

type emulatorRuntimeDependencies struct {
	dataPlaneRegistry dataplaneapp.Registry
	configctlRegistry adapternats.ConfigctlRegistry
}

func Run(config settings.AppConfig) {
	logger := bootstrap.BuildLogger(config.Log)
	slog.SetDefault(logger)

	if prob := validateEmulatorConfig(config); prob != nil {
		logger.Error("invalid emulator config", "error", prob)
		os.Exit(1)
	}

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	deps := defaultEmulatorRuntimeDependencies()
	bootstrapState, prob := runtimebootstrap.WaitForConfiguredActiveIngestionBootstrapSetWithRegistry(ctx, logger, config, "emulator", deps.dataPlaneRegistry)
	if prob != nil {
		logger.Error("bootstrap active ingestion bindings", "error", prob)
		os.Exit(1)
	}
	bootstrapSignature := bootstrapState.Signature()
	refreshSignals := make(chan struct{}, 1)
	runtimeChangedConsumer := adapternats.NewIngestionRuntimeChangedConsumer(config.NATS.URL, deps.configctlRegistry.EmulatorRuntimeChanged, &emulatorRefreshNotifier{signals: refreshSignals})
	if err := runtimeChangedConsumer.Start(); err != nil {
		logger.Error("start emulator runtime refresh consumer", "error", err)
		os.Exit(1)
	}
	defer func() {
		if err := runtimeChangedConsumer.Close(); err != nil {
			logger.Error("close emulator runtime refresh consumer", "error", err)
		}
	}()
	if err := ensureTopicsForBootstrap(ctx, config.Kafka.Brokers, bootstrapState.Index.Topics(), config.Kafka.DialTimeoutDuration()); err != nil {
		logger.Error("ensure kafka topics", "error", err)
		os.Exit(1)
	}

	clientID := strings.TrimSpace(config.Kafka.ClientID)
	if clientID == "" {
		clientID = "quality-service-emulator"
	}

	producer, err := adapterkafka.NewProducer(config.Kafka.Brokers, clientID, config.Kafka.DialTimeoutDuration())
	if err != nil {
		logger.Error("create kafka producer", "error", err)
		os.Exit(1)
	}
	defer func() {
		if err := producer.Close(); err != nil {
			logger.Error("close kafka producer", "error", err)
		}
	}()

	logger.Info("emulator started", "topics", bootstrapState.Index.Topics(), "bindings", len(bootstrapState.Index.All()), "bootstrap_signature", bootstrapSignature, "runtime_refs", bootstrapState.RuntimeRefs())

	ticker := time.NewTicker(config.Emulator.PublishIntervalDuration())
	defer ticker.Stop()

	var reconcileTicker *time.Ticker
	if interval := config.Bootstrap.ReconcileIntervalDuration(); interval > 0 {
		reconcileTicker = time.NewTicker(interval)
		defer reconcileTicker.Stop()
	}

	var reconcileTick <-chan time.Time
	if reconcileTicker != nil {
		reconcileTick = reconcileTicker.C
	}

	var sequence int64
	if err := publishSyntheticBatch(ctx, logger, producer, deps.dataPlaneRegistry, bootstrapState.Index, &sequence); err != nil {
		logger.Error("publish synthetic batch", "error", err)
		os.Exit(1)
	}

	for {
		select {
		case <-ctx.Done():
			logger.Info("emulator stopped")
			return
		case <-refreshSignals:
			bootstrapState, bootstrapSignature = reconcileBootstrapState(ctx, logger, config, deps.dataPlaneRegistry, bootstrapState, bootstrapSignature)
		case <-reconcileTick:
			bootstrapState, bootstrapSignature = reconcileBootstrapState(ctx, logger, config, deps.dataPlaneRegistry, bootstrapState, bootstrapSignature)
		case <-ticker.C:
			if err := publishSyntheticBatch(ctx, logger, producer, deps.dataPlaneRegistry, bootstrapState.Index, &sequence); err != nil {
				logger.Error("publish synthetic batch", "error", err)
				os.Exit(1)
			}
		}
	}
}

func defaultEmulatorRuntimeDependencies() emulatorRuntimeDependencies {
	return emulatorRuntimeDependencies{
		dataPlaneRegistry: dataplaneapp.DefaultRegistry(),
		configctlRegistry: adapternats.DefaultConfigctlRegistry(),
	}
}

func reconcileBootstrapState(ctx context.Context, logger *slog.Logger, config settings.AppConfig, registry dataplaneapp.Registry, current runtimebootstrap.ActiveIngestionBootstrap, currentSignature string) (runtimebootstrap.ActiveIngestionBootstrap, string) {
	refreshed, changed, refreshErr := refreshBootstrapState(ctx, logger, config, registry, currentSignature)
	if refreshErr != nil {
		logger.Warn("refresh emulator bootstrap", "error", refreshErr)
		return current, currentSignature
	}
	if !changed {
		return current, currentSignature
	}
	if err := ensureTopicsForBootstrap(ctx, config.Kafka.Brokers, refreshed.Index.Topics(), config.Kafka.DialTimeoutDuration()); err != nil {
		logger.Warn("ensure kafka topics for refreshed bootstrap", "error", err)
		return current, currentSignature
	}

	logger.Info("emulator bootstrap refreshed", "topics", refreshed.Index.Topics(), "bindings", len(refreshed.Index.All()), "bootstrap_signature", refreshed.Signature(), "runtime_refs", refreshed.RuntimeRefs())
	return refreshed, refreshed.Signature()
}

func refreshBootstrapState(ctx context.Context, logger *slog.Logger, config settings.AppConfig, registry dataplaneapp.Registry, currentSignature string) (runtimebootstrap.ActiveIngestionBootstrap, bool, *problem.Problem) {
	bootstrapState, prob := runtimebootstrap.WaitForConfiguredActiveIngestionBootstrapSetWithRegistry(ctx, logger, config, "emulator", registry)
	if prob != nil {
		return runtimebootstrap.ActiveIngestionBootstrap{}, false, prob
	}

	nextSignature := bootstrapState.Signature()
	return bootstrapState, nextSignature != currentSignature, nil
}

func validateEmulatorConfig(config settings.AppConfig) *problem.Problem {
	var issues []problem.ValidationIssue
	if !config.Kafka.Enabled {
		issues = append(issues, problem.ValidationIssue{Field: "kafka.enabled", Message: "must be true for emulator"})
	}
	if !config.NATS.Enabled {
		issues = append(issues, problem.ValidationIssue{Field: "nats.enabled", Message: "must be true for emulator"})
	}
	if strings.TrimSpace(config.NATS.URL) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "nats.url", Message: "must not be empty for emulator"})
	}
	if strings.TrimSpace(config.Bootstrap.BaseURL) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "bootstrap.base_url", Message: "must not be empty for emulator"})
	}
	if len(config.Kafka.Brokers) == 0 {
		issues = append(issues, problem.ValidationIssue{Field: "kafka.brokers", Message: "must contain at least one broker for emulator"})
	}
	if len(issues) == 0 {
		return nil
	}
	return problem.Validation(problem.InvalidArgument, "emulator config is invalid", issues...)
}

type emulatorRefreshNotifier struct {
	signals chan<- struct{}
}

func (n *emulatorRefreshNotifier) HandleIngestionRuntimeChanged(_ context.Context, _ configdomain.IngestionRuntimeChangedEvent) *problem.Problem {
	if n == nil || n.signals == nil {
		return problem.New(problem.Unavailable, "emulator refresh channel is unavailable")
	}
	select {
	case n.signals <- struct{}{}:
	default:
	}
	return nil
}

func publishSyntheticBatch(ctx context.Context, logger *slog.Logger, producer *adapterkafka.Producer, registry dataplaneapp.Registry, index dataplaneapp.BindingIndex, sequence *int64) error {
	now := time.Now().UTC()
	var published int
	for _, binding := range index.All() {
		for _, scenario := range []dataplaneapp.SyntheticScenario{
			dataplaneapp.SyntheticScenarioValid,
			dataplaneapp.SyntheticScenarioInvalidMissingField,
		} {
			*sequence++
			route, prob := registry.RouteForBinding(binding)
			if prob != nil {
				return fmt.Errorf("resolve binding route: %w", prob)
			}
			record, prob := dataplaneapp.BuildSyntheticRecord(binding, dataplaneapp.SyntheticInput{
				Now:      now,
				Sequence: *sequence,
				Scenario: scenario,
			})
			if prob != nil {
				return fmt.Errorf("build synthetic record: %w", prob)
			}

			if err := producer.Publish(ctx, route.KafkaTopic, []byte(record.Key), record.Payload, map[string]string{
				"content-type":         dataplaneapp.ContentTypeJSON,
				"x-correlation-id":     record.Key,
				"x-synthetic-scenario": string(record.Scenario),
			}, now); err != nil {
				return fmt.Errorf("publish kafka message: %w", err)
			}
			published++
		}
	}

	logger.Info("published synthetic batch", "messages", published, "at", now)
	return nil
}
