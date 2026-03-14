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
	dataplaneapp "internal/application/dataplane"
	runtimebootstrap "internal/application/runtimebootstrap"
	"internal/shared/bootstrap"
	"internal/shared/problem"
	"internal/shared/settings"
)

func Run(config settings.AppConfig) {
	logger := bootstrap.BuildLogger(config.Log)
	slog.SetDefault(logger)

	if prob := validateEmulatorConfig(config); prob != nil {
		logger.Error("invalid emulator config", "error", prob)
		os.Exit(1)
	}

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	bootstrapState, prob := runtimebootstrap.WaitForConfiguredActiveIngestionBootstrap(ctx, logger, config, "emulator")
	if prob != nil {
		logger.Error("bootstrap active ingestion bindings", "error", prob)
		os.Exit(1)
	}

	registry := dataplaneapp.DefaultRegistry()
	if err := adapterkafka.EnsureTopics(ctx, config.Kafka.Brokers, bootstrapState.Index.Topics(), config.Kafka.DialTimeoutDuration()); err != nil {
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

	logger.Info("emulator started", "topics", bootstrapState.Index.Topics(), "bindings", len(bootstrapState.Index.All()))

	ticker := time.NewTicker(config.Emulator.PublishIntervalDuration())
	defer ticker.Stop()

	var sequence int64
	if err := publishSyntheticBatch(ctx, logger, producer, registry, bootstrapState.Index, &sequence); err != nil {
		logger.Error("publish synthetic batch", "error", err)
		os.Exit(1)
	}

	for {
		select {
		case <-ctx.Done():
			logger.Info("emulator stopped")
			return
		case <-ticker.C:
			if err := publishSyntheticBatch(ctx, logger, producer, registry, bootstrapState.Index, &sequence); err != nil {
				logger.Error("publish synthetic batch", "error", err)
				os.Exit(1)
			}
		}
	}
}

func validateEmulatorConfig(config settings.AppConfig) *problem.Problem {
	var issues []problem.ValidationIssue
	if !config.Kafka.Enabled {
		issues = append(issues, problem.ValidationIssue{Field: "kafka.enabled", Message: "must be true for emulator"})
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
