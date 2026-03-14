package consumer

import (
	"context"
	"log/slog"
	"testing"
	"time"

	actorcommon "internal/actors/common"
	configctlcontracts "internal/application/configctl/contracts"
	dataplaneapp "internal/application/dataplane"
	runtimebootstrap "internal/application/runtimebootstrap"
	sharedruntime "internal/application/runtimecontracts"
	"internal/shared/problem"
	"internal/shared/settings"

	"github.com/anthdm/hollywood/actor"
)

type readyRuntimeActor struct {
	cfg ConsumerRuntimeConfig
}

func (a *readyRuntimeActor) Receive(c *actor.Context) {
	switch c.Message().(type) {
	case actor.Started:
		c.Send(c.Parent(), consumerRuntimeReadyMessage{
			Generation: a.cfg.Generation,
			Topology:   a.cfg.Bootstrap.Topology,
		})
	}
}

func TestSupervisorReplacesRuntimeGenerationOnBootstrapReload(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	supervisorPID := engine.Spawn(newSupervisorProducer(supervisorConfig{
		appConfig: settings.AppConfig{},
		registry:  dataplaneapp.DefaultRegistry(),
		loadBootstrap: func(ctx context.Context, _ *slog.Logger, _ settings.AppConfig, _ string) (runtimebootstrap.ActiveIngestionBootstrap, *problem.Problem) {
			<-ctx.Done()
			return runtimebootstrap.ActiveIngestionBootstrap{}, problem.Wrap(ctx.Err(), problem.Unavailable, "bootstrap stopped")
		},
		newRuntimeActor: func(cfg ConsumerRuntimeConfig) actor.Producer {
			return func() actor.Receiver { return &readyRuntimeActor{cfg: cfg} }
		},
	}), "consumer-supervisor-test")

	first := mustBootstrap(t, "orders", "sales.order.created", "cfg-1", "global", "default")
	second := mustBootstrap(t, "payments", "sales.payment.created", "cfg-2", "tenant", "br")

	engine.Send(supervisorPID, activeIngestionBootstrapLoadedMessage{Bootstrap: first})
	state := awaitConsumerState(t, engine, supervisorPID, func(state ConsumerSupervisorState) bool {
		return state.Ready && state.Generation == 1 && len(state.Topics) == 1 && state.Topics[0] == "sales.order.created"
	})
	if state.Bindings != 1 {
		t.Fatalf("expected one binding in first generation, got %+v", state)
	}

	engine.Send(supervisorPID, activeIngestionBootstrapLoadedMessage{Bootstrap: second})
	state = awaitConsumerState(t, engine, supervisorPID, func(state ConsumerSupervisorState) bool {
		return state.Ready && state.Generation == 2 && len(state.Topics) == 1 && state.Topics[0] == "sales.payment.created"
	})
	if state.Bindings != 1 {
		t.Fatalf("expected one binding in second generation, got %+v", state)
	}
}

func mustBootstrap(t *testing.T, name, topic, versionID, scopeKind, scopeKey string) runtimebootstrap.ActiveIngestionBootstrap {
	t.Helper()

	index, prob := dataplaneapp.NewBindingIndex([]configctlcontracts.ActiveIngestionBindingRecord{
		{
			Binding: configctlcontracts.BindingRecord{Name: name, Topic: topic},
			Runtime: sharedruntime.RuntimeRecord{
				Scope:  sharedruntime.ScopeRecord{Kind: scopeKind, Key: scopeKey},
				Config: sharedruntime.ConfigRecord{VersionID: versionID},
			},
		},
	})
	if prob != nil {
		t.Fatalf("new binding index: %v", prob)
	}

	topology, prob := dataplaneapp.NewRuntimeTopology(index, dataplaneapp.DefaultRegistry())
	if prob != nil {
		t.Fatalf("new runtime topology: %v", prob)
	}

	return runtimebootstrap.ActiveIngestionBootstrap{
		Index:    index,
		Topology: topology,
	}
}

func awaitConsumerState(t *testing.T, engine *actor.Engine, supervisorPID *actor.PID, match func(ConsumerSupervisorState) bool) ConsumerSupervisorState {
	t.Helper()

	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		rawResult, err := engine.Request(supervisorPID, queryConsumerSupervisorStateMessage{}, time.Second).Result()
		if err != nil {
			t.Fatalf("query supervisor state: %v", err)
		}

		state := rawResult.(queryConsumerSupervisorStateResult).State
		if match(state) {
			return state
		}

		time.Sleep(10 * time.Millisecond)
	}

	t.Fatal("consumer supervisor state did not converge")
	return ConsumerSupervisorState{}
}
