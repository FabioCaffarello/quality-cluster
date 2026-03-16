package validator

import (
	"context"
	"fmt"
	"log/slog"

	actorcommon "internal/actors/common"
	adapternats "internal/adapters/nats"
	configctlcontracts "internal/application/configctl/contracts"
	configctlclient "internal/application/configctlclient"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"
	"internal/shared/settings"

	"github.com/anthdm/hollywood/actor"
)

type runtimeProjectionLoaderFunc func(context.Context, settings.AppConfig) ([]configdomain.RuntimeProjection, *problem.Problem)
type runtimeCacheFactory func() actor.Producer
type resultsStoreFactory func() actor.Producer
type validationRouterFactory func(ValidationRouterConfig) actor.Producer
type runtimeConsumerFactory func(RuntimeConsumerConfig) actor.Producer
type dataPlaneConsumerFactory func(DataPlaneConsumerConfig) actor.Producer
type runtimeQueryResponderFactory func(RuntimeQueryResponderConfig) actor.Producer
type resultsQueryResponderFactory func(ResultsQueryResponderConfig) actor.Producer
type incidentsQueryResponderFactory func(IncidentsQueryResponderConfig) actor.Producer

type supervisorConfig struct {
	appConfig settings.AppConfig

	configctlRegistry adapternats.ConfigctlRegistry
	dataPlaneRegistry adapternats.DataPlaneRegistry
	runtimeRegistry   adapternats.ValidatorRuntimeRegistry
	resultsRegistry   adapternats.ValidatorResultsRegistry
	incidentsRegistry adapternats.ValidatorIncidentsRegistry

	loadRuntimeProjections          runtimeProjectionLoaderFunc
	newRuntimeCacheActor            runtimeCacheFactory
	newResultsStoreActor            resultsStoreFactory
	newValidationRouterActor        validationRouterFactory
	newRuntimeConsumerActor         runtimeConsumerFactory
	newDataPlaneConsumerActor       dataPlaneConsumerFactory
	newRuntimeQueryResponderActor   runtimeQueryResponderFactory
	newResultsQueryResponderActor   resultsQueryResponderFactory
	newIncidentsQueryResponderActor incidentsQueryResponderFactory
}

type Supervisor struct {
	cfg    supervisorConfig
	logger *slog.Logger
}

func NewSupervisor(cfg settings.AppConfig) actor.Producer {
	return newSupervisorProducer(supervisorConfig{
		appConfig:                       cfg,
		configctlRegistry:               adapternats.DefaultConfigctlRegistry(),
		dataPlaneRegistry:               adapternats.DefaultDataPlaneRegistry(),
		runtimeRegistry:                 adapternats.DefaultValidatorRuntimeRegistry(),
		resultsRegistry:                 adapternats.DefaultValidatorResultsRegistry(),
		incidentsRegistry:               adapternats.DefaultValidatorIncidentsRegistry(),
		loadRuntimeProjections:          loadRuntimeProjectionsFromConfigctl,
		newRuntimeCacheActor:            NewRuntimeCacheActor,
		newResultsStoreActor:            NewValidationResultsStoreActor,
		newValidationRouterActor:        NewValidationRouterActor,
		newRuntimeConsumerActor:         NewRuntimeConsumerActor,
		newDataPlaneConsumerActor:       NewDataPlaneConsumerActor,
		newRuntimeQueryResponderActor:   NewRuntimeQueryResponderActor,
		newResultsQueryResponderActor:   NewResultsQueryResponderActor,
		newIncidentsQueryResponderActor: NewIncidentsQueryResponderActor,
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

func (s *Supervisor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		if err := s.start(c); err != nil {
			s.logger.Error("start validator supervisor", "error", err)
			c.Engine().Poison(c.PID())
		}
	case actor.Stopped:
	default:
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
		s.logger.Warn("validator supervisor: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (s *Supervisor) start(c *actor.Context) error {
	if prob := ValidateConfig(s.cfg.appConfig); prob != nil {
		return prob
	}

	cachePID := c.SpawnChild(s.cfg.newRuntimeCacheActor(), "runtime-cache")
	if err := s.bootstrapRuntimeCache(c, cachePID); err != nil {
		return err
	}

	resultsPID := c.SpawnChild(s.cfg.newResultsStoreActor(), "results-store")
	routerPID := c.SpawnChild(s.cfg.newValidationRouterActor(ValidationRouterConfig{
		RuntimeCachePID: cachePID,
		ResultsStorePID: resultsPID,
		RequestTimeout:  s.cfg.appConfig.NATS.RequestTimeoutDuration(),
	}), "validation-router")
	c.SpawnChild(s.cfg.newRuntimeConsumerActor(RuntimeConsumerConfig{
		URL:      s.cfg.appConfig.NATS.URL,
		Registry: s.cfg.configctlRegistry,
		CachePID: cachePID,
	}), "runtime-consumer")
	c.SpawnChild(s.cfg.newDataPlaneConsumerActor(DataPlaneConsumerConfig{
		URL:            s.cfg.appConfig.NATS.URL,
		Registry:       s.cfg.dataPlaneRegistry,
		RouterPID:      routerPID,
		RequestTimeout: s.cfg.appConfig.NATS.RequestTimeoutDuration(),
	}), "dataplane-consumer")
	c.SpawnChild(s.cfg.newRuntimeQueryResponderActor(RuntimeQueryResponderConfig{
		URL:             s.cfg.appConfig.NATS.URL,
		Source:          "validator.runtime",
		Registry:        s.cfg.runtimeRegistry,
		RuntimeCachePID: cachePID,
		RequestTimeout:  s.cfg.appConfig.NATS.RequestTimeoutDuration(),
	}), "runtime-query-responder")
	c.SpawnChild(s.cfg.newResultsQueryResponderActor(ResultsQueryResponderConfig{
		URL:             s.cfg.appConfig.NATS.URL,
		Source:          "validator.results",
		Registry:        s.cfg.resultsRegistry,
		ResultsStorePID: resultsPID,
		RequestTimeout:  s.cfg.appConfig.NATS.RequestTimeoutDuration(),
	}), "results-query-responder")
	c.SpawnChild(s.cfg.newIncidentsQueryResponderActor(IncidentsQueryResponderConfig{
		URL:             s.cfg.appConfig.NATS.URL,
		Source:          "validator.incidents",
		Registry:        s.cfg.incidentsRegistry,
		ResultsStorePID: resultsPID,
		RequestTimeout:  s.cfg.appConfig.NATS.RequestTimeoutDuration(),
	}), "incidents-query-responder")

	s.logger.Info("validator started")
	return nil
}

func (s *Supervisor) bootstrapRuntimeCache(c *actor.Context, cachePID *actor.PID) error {
	projections, prob := s.cfg.loadRuntimeProjections(context.Background(), s.cfg.appConfig)
	if prob != nil {
		return prob
	}

	for _, projection := range projections {
		c.Engine().Send(cachePID, bootstrapRuntimeProjectionMessage{
			Projection: projection,
		})
	}
	s.logger.Info("validator runtime bootstrap loaded", "runtimes", len(projections))
	return nil
}

func loadRuntimeProjectionsFromConfigctl(ctx context.Context, cfg settings.AppConfig) ([]configdomain.RuntimeProjection, *problem.Problem) {
	requestClient, err := adapternats.NewNATSRequestClientWithURL(cfg.NATS.URL, cfg.NATS.RequestTimeoutDuration())
	if err != nil {
		return nil, problem.Wrap(err, problem.Unavailable, "create validator bootstrap request client")
	}
	defer requestClient.Close()

	gateway := adapternats.NewConfigctlGateway(requestClient, "validator.bootstrap")
	reply, prob := configctlclient.NewListActiveRuntimeProjectionsUseCase(gateway).Execute(ctx, configctlcontracts.ListActiveRuntimeProjectionsQuery{})
	if prob != nil {
		return nil, prob
	}

	projections := make([]configdomain.RuntimeProjection, 0, len(reply.Runtimes))
	for _, runtime := range reply.Runtimes {
		projections = append(projections, runtimeProjectionFromRecord(runtime))
	}
	return projections, nil
}
