package consumer

import (
	"context"
	"log/slog"
	"sync"
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

type bootstrapProbeActor struct {
	cfg bootstrapActorConfig
	ch  chan<- activeIngestionBootstrapLoadedMessage
}

func (a *bootstrapProbeActor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		c.SpawnChild(newBootstrapActor(a.cfg), "bootstrap")
	case activeIngestionBootstrapLoadedMessage:
		a.ch <- msg
	case activeIngestionBootstrapFailedMessage:
		panic("bootstrap probe received failure: " + msg.Prob.Error())
	}
}

func TestBootstrapActorPeriodicallyReconcilesWithoutRuntimeChangeEvent(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	var (
		mu      sync.Mutex
		loads   int
		first   = mustActiveBootstrap(t, "orders", "sales.order.created", "cfg-1", "global", "default")
		second  = mustActiveBootstrap(t, "payments", "sales.payment.created", "cfg-2", "tenant", "br")
		updates = make(chan activeIngestionBootstrapLoadedMessage, 2)
	)

	parent := engine.Spawn(func() actor.Receiver {
		return &bootstrapProbeActor{
			cfg: bootstrapActorConfig{
				appConfig:         settings.AppConfig{},
				reconcileInterval: 10 * time.Millisecond,
				loadBootstrap: func(context.Context, *slog.Logger, settings.AppConfig, string) (runtimebootstrap.ActiveIngestionBootstrap, *problem.Problem) {
					mu.Lock()
					defer mu.Unlock()
					loads++
					if loads == 1 {
						return first, nil
					}
					return second, nil
				},
			},
			ch: updates,
		}
	}, "bootstrap-probe-parent")
	defer engine.Poison(parent)

	gotFirst := awaitBootstrapUpdate(t, updates)
	if gotFirst.Bootstrap.Signature() != first.Signature() {
		t.Fatalf("expected first bootstrap signature %q, got %q", first.Signature(), gotFirst.Bootstrap.Signature())
	}

	gotSecond := awaitBootstrapUpdate(t, updates)
	if gotSecond.Bootstrap.Signature() != second.Signature() {
		t.Fatalf("expected reconciled bootstrap signature %q, got %q", second.Signature(), gotSecond.Bootstrap.Signature())
	}
}

func awaitBootstrapUpdate(t *testing.T, ch <-chan activeIngestionBootstrapLoadedMessage) activeIngestionBootstrapLoadedMessage {
	t.Helper()

	select {
	case msg := <-ch:
		return msg
	case <-time.After(2 * time.Second):
		t.Fatal("bootstrap update did not arrive")
		return activeIngestionBootstrapLoadedMessage{}
	}
}

func mustActiveBootstrap(t *testing.T, name, topic, versionID, scopeKind, scopeKey string) runtimebootstrap.ActiveIngestionBootstrap {
	t.Helper()

	bindings := []configctlcontracts.ActiveIngestionBindingRecord{
		{
			Binding: configctlcontracts.BindingRecord{Name: name, Topic: topic},
			Runtime: sharedruntime.RuntimeRecord{
				Scope:  sharedruntime.ScopeRecord{Kind: scopeKind, Key: scopeKey},
				Config: sharedruntime.ConfigRecord{VersionID: versionID},
			},
		},
	}

	index, prob := dataplaneapp.NewBindingIndex(bindings)
	if prob != nil {
		t.Fatalf("new binding index: %v", prob)
	}

	topology, prob := dataplaneapp.NewRuntimeTopology(index, dataplaneapp.DefaultRegistry())
	if prob != nil {
		t.Fatalf("new runtime topology: %v", prob)
	}

	return runtimebootstrap.ActiveIngestionBootstrap{
		Bindings: bindings,
		Index:    index,
		Topology: topology,
	}
}
