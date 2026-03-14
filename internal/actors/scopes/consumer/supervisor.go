package consumer

import (
	"context"
	"fmt"
	"log/slog"
	"strings"

	dataplaneapp "internal/application/dataplane"
	runtimebootstrap "internal/application/runtimebootstrap"
	"internal/shared/problem"
	"internal/shared/settings"

	"github.com/anthdm/hollywood/actor"
)

type bootstrapLoaderFunc func(context.Context, *slog.Logger, settings.AppConfig, string) (runtimebootstrap.ActiveIngestionBootstrap, *problem.Problem)
type runtimeProducerFactory func(ConsumerRuntimeConfig) actor.Producer

type supervisorConfig struct {
	appConfig       settings.AppConfig
	registry        dataplaneapp.Registry
	loadBootstrap   bootstrapLoaderFunc
	newRuntimeActor runtimeProducerFactory
}

type Supervisor struct {
	cfg        supervisorConfig
	logger     *slog.Logger
	runtimePID *actor.PID
	state      ConsumerSupervisorState
}

func NewSupervisor(appConfig settings.AppConfig) actor.Producer {
	return newSupervisorProducer(supervisorConfig{
		appConfig:       appConfig,
		registry:        dataplaneapp.DefaultRegistry(),
		loadBootstrap:   runtimebootstrap.WaitForConfiguredActiveIngestionBootstrap,
		newRuntimeActor: NewConsumerRuntimeActor,
	})
}

func newSupervisorProducer(cfg supervisorConfig) actor.Producer {
	return func() actor.Receiver {
		return &Supervisor{
			cfg:    cfg,
			logger: slog.Default(),
		}
	}
}

func ValidateConfig(config settings.AppConfig) *problem.Problem {
	var issues []problem.ValidationIssue
	if !config.Kafka.Enabled {
		issues = append(issues, problem.ValidationIssue{Field: "kafka.enabled", Message: "must be true for consumer"})
	}
	if !config.NATS.Enabled {
		issues = append(issues, problem.ValidationIssue{Field: "nats.enabled", Message: "must be true for consumer"})
	}
	if strings.TrimSpace(config.Bootstrap.BaseURL) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "bootstrap.base_url", Message: "must not be empty for consumer"})
	}
	if len(config.Kafka.Brokers) == 0 {
		issues = append(issues, problem.ValidationIssue{Field: "kafka.brokers", Message: "must contain at least one broker for consumer"})
	}
	if len(issues) == 0 {
		return nil
	}
	return problem.Validation(problem.InvalidArgument, "consumer config is invalid", issues...)
}

func (s *Supervisor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		c.SpawnChild(newBootstrapActor(bootstrapActorConfig{
			appConfig:     s.cfg.appConfig,
			loadBootstrap: s.cfg.loadBootstrap,
		}), "bootstrap")
	case activeIngestionBootstrapLoadedMessage:
		s.startRuntime(c, msg.Bootstrap)
	case activeIngestionBootstrapFailedMessage:
		s.logger.Error("bootstrap consumer runtime", "error", msg.Prob)
		c.Engine().Poison(c.PID())
	case consumerRuntimeReadyMessage:
		s.state.Generation = msg.Generation
		s.state.Ready = true
		s.state.Topics = msg.Topology.TopicNames()
		s.state.Bindings = msg.Topology.BindingCount()
		s.logger.Info("consumer runtime ready", "generation", msg.Generation, "topics", s.state.Topics, "bindings", s.state.Bindings)
	case consumerRuntimeFailedMessage:
		s.state.Ready = false
		s.logger.Error("consumer runtime failed", "generation", msg.Generation, "error", msg.Err)
		c.Engine().Poison(c.PID())
	case queryConsumerSupervisorStateMessage:
		c.Respond(queryConsumerSupervisorStateResult{State: s.state})
	case actor.Stopped:
	default:
		s.logger.Warn("consumer supervisor: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (s *Supervisor) startRuntime(c *actor.Context, bootstrap runtimebootstrap.ActiveIngestionBootstrap) {
	s.state.Ready = false
	s.state.Topics = nil
	s.state.Bindings = 0

	if s.runtimePID != nil {
		_ = c.Engine().Poison(s.runtimePID)
	}

	s.state.Generation++
	generation := s.state.Generation
	s.runtimePID = c.SpawnChild(s.cfg.newRuntimeActor(ConsumerRuntimeConfig{
		AppConfig:  s.cfg.appConfig,
		Generation: generation,
		Bootstrap:  bootstrap,
		Registry:   s.cfg.registry,
		Source:     "consumer.dataplane",
	}), fmt.Sprintf("runtime-%d", generation))
}
