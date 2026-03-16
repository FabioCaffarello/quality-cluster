package validator

import (
	"context"
	"reflect"
	"testing"
	"time"

	actorcommon "internal/actors/common"
	adapternats "internal/adapters/nats"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"
	"internal/shared/settings"

	"github.com/anthdm/hollywood/actor"
)

type noopActor struct{}

func (a *noopActor) Receive(*actor.Context) {}

type bootstrapCaptureActor struct {
	bootstraps chan<- bootstrapRuntimeProjectionMessage
}

func (a *bootstrapCaptureActor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case bootstrapRuntimeProjectionMessage:
		a.bootstraps <- msg
	}
}

func TestSupervisorBootstrapsRuntimeCacheFromInjectedLoader(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	expected := configdomain.RuntimeProjection{
		Scope:       configdomain.ActivationScope{Kind: "global", Key: "default"},
		ConfigSetID: "set-1",
		VersionID:   "cfg-1",
	}
	bootstraps := make(chan bootstrapRuntimeProjectionMessage, 1)

	supervisorPID := engine.Spawn(newSupervisorProducer(supervisorConfig{
		appConfig: settings.AppConfig{
			NATS: settings.NATSConfig{Enabled: true},
		},
		loadRuntimeProjections: func(context.Context, settings.AppConfig) ([]configdomain.RuntimeProjection, *problem.Problem) {
			return []configdomain.RuntimeProjection{expected}, nil
		},
		newRuntimeCacheActor: func() actor.Producer {
			return func() actor.Receiver { return &bootstrapCaptureActor{bootstraps: bootstraps} }
		},
		newResultsStoreActor: func() actor.Producer {
			return func() actor.Receiver { return &noopActor{} }
		},
		newValidationRouterActor: func(ValidationRouterConfig) actor.Producer {
			return func() actor.Receiver { return &noopActor{} }
		},
		newRuntimeConsumerActor: func(RuntimeConsumerConfig) actor.Producer {
			return func() actor.Receiver { return &noopActor{} }
		},
		newDataPlaneConsumerActor: func(DataPlaneConsumerConfig) actor.Producer {
			return func() actor.Receiver { return &noopActor{} }
		},
		newRuntimeQueryResponderActor: func(RuntimeQueryResponderConfig) actor.Producer {
			return func() actor.Receiver { return &noopActor{} }
		},
		newResultsQueryResponderActor: func(ResultsQueryResponderConfig) actor.Producer {
			return func() actor.Receiver { return &noopActor{} }
		},
		newIncidentsQueryResponderActor: func(IncidentsQueryResponderConfig) actor.Producer {
			return func() actor.Receiver { return &noopActor{} }
		},
	}), "validator-supervisor-bootstrap-test")
	defer engine.Poison(supervisorPID)

	select {
	case msg := <-bootstraps:
		if !reflect.DeepEqual(msg.Projection, expected) {
			t.Fatalf("expected bootstrap projection %#v, got %#v", expected, msg.Projection)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("bootstrap projection did not reach runtime cache")
	}
}

func TestSupervisorPassesExplicitRegistriesToChildActors(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	defaultDataPlane := adapternats.DefaultDataPlaneRegistry()
	defaultRuntime := adapternats.DefaultValidatorRuntimeRegistry()
	defaultResults := adapternats.DefaultValidatorResultsRegistry()
	defaultIncidents := adapternats.DefaultValidatorIncidentsRegistry()
	defaultConfigctl := adapternats.DefaultConfigctlRegistry()

	customConfigctl := defaultConfigctl
	customConfigctl.ValidatorRuntime.Durable = "validator-runtime-custom"
	customDataPlane := defaultDataPlane
	customDataPlane.ValidatorIngested.Durable = "validator-ingested-custom"
	customRuntime := defaultRuntime
	customRuntime.GetActive.Subject = "validator.runtime.active.custom"
	customResults := defaultResults
	customResults.List.Subject = "validator.results.list.custom"
	customIncidents := defaultIncidents
	customIncidents.List.Subject = "validator.incidents.list.custom"

	runtimeConsumerConfigs := make(chan RuntimeConsumerConfig, 1)
	dataPlaneConsumerConfigs := make(chan DataPlaneConsumerConfig, 1)
	runtimeResponderConfigs := make(chan RuntimeQueryResponderConfig, 1)
	resultsResponderConfigs := make(chan ResultsQueryResponderConfig, 1)
	incidentsResponderConfigs := make(chan IncidentsQueryResponderConfig, 1)

	supervisorPID := engine.Spawn(newSupervisorProducer(supervisorConfig{
		appConfig: settings.AppConfig{
			NATS: settings.NATSConfig{Enabled: true, URL: "nats://example"},
		},
		configctlRegistry: customConfigctl,
		dataPlaneRegistry: customDataPlane,
		runtimeRegistry:   customRuntime,
		resultsRegistry:   customResults,
		incidentsRegistry: customIncidents,
		loadRuntimeProjections: func(context.Context, settings.AppConfig) ([]configdomain.RuntimeProjection, *problem.Problem) {
			return nil, nil
		},
		newRuntimeCacheActor: func() actor.Producer {
			return func() actor.Receiver { return &noopActor{} }
		},
		newResultsStoreActor: func() actor.Producer {
			return func() actor.Receiver { return &noopActor{} }
		},
		newValidationRouterActor: func(ValidationRouterConfig) actor.Producer {
			return func() actor.Receiver { return &noopActor{} }
		},
		newRuntimeConsumerActor: func(cfg RuntimeConsumerConfig) actor.Producer {
			runtimeConsumerConfigs <- cfg
			return func() actor.Receiver { return &noopActor{} }
		},
		newDataPlaneConsumerActor: func(cfg DataPlaneConsumerConfig) actor.Producer {
			dataPlaneConsumerConfigs <- cfg
			return func() actor.Receiver { return &noopActor{} }
		},
		newRuntimeQueryResponderActor: func(cfg RuntimeQueryResponderConfig) actor.Producer {
			runtimeResponderConfigs <- cfg
			return func() actor.Receiver { return &noopActor{} }
		},
		newResultsQueryResponderActor: func(cfg ResultsQueryResponderConfig) actor.Producer {
			resultsResponderConfigs <- cfg
			return func() actor.Receiver { return &noopActor{} }
		},
		newIncidentsQueryResponderActor: func(cfg IncidentsQueryResponderConfig) actor.Producer {
			incidentsResponderConfigs <- cfg
			return func() actor.Receiver { return &noopActor{} }
		},
	}), "validator-supervisor-registry-test")
	defer engine.Poison(supervisorPID)

	assertEqualWithin(t, runtimeConsumerConfigs, func(cfg RuntimeConsumerConfig) bool {
		return reflect.DeepEqual(cfg.Registry, customConfigctl)
	}, "runtime consumer registry")
	assertEqualWithin(t, dataPlaneConsumerConfigs, func(cfg DataPlaneConsumerConfig) bool {
		return reflect.DeepEqual(cfg.Registry, customDataPlane)
	}, "data plane consumer registry")
	assertEqualWithin(t, runtimeResponderConfigs, func(cfg RuntimeQueryResponderConfig) bool {
		return reflect.DeepEqual(cfg.Registry, customRuntime)
	}, "runtime responder registry")
	assertEqualWithin(t, resultsResponderConfigs, func(cfg ResultsQueryResponderConfig) bool {
		return reflect.DeepEqual(cfg.Registry, customResults)
	}, "results responder registry")
	assertEqualWithin(t, incidentsResponderConfigs, func(cfg IncidentsQueryResponderConfig) bool {
		return reflect.DeepEqual(cfg.Registry, customIncidents)
	}, "incidents responder registry")
}

func assertEqualWithin[T any](t *testing.T, ch <-chan T, match func(T) bool, label string) {
	t.Helper()

	select {
	case got := <-ch:
		if !match(got) {
			t.Fatalf("unexpected %s: %#v", label, got)
		}
	case <-time.After(2 * time.Second):
		t.Fatalf("%s was not captured", label)
	}
}
