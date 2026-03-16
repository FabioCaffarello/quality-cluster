package consumer

import (
	"strings"
	"testing"
	"time"

	actorcommon "internal/actors/common"
	adapternats "internal/adapters/nats"
	runtimebootstrap "internal/application/runtimebootstrap"
	"internal/shared/settings"

	"github.com/anthdm/hollywood/actor"
)

type runtimeProbeActor struct {
	cfg    ConsumerRuntimeConfig
	failed chan<- consumerRuntimeFailedMessage
}

func (a *runtimeProbeActor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		c.SpawnChild(NewConsumerRuntimeActor(a.cfg), "runtime")
	case consumerRuntimeFailedMessage:
		a.failed <- msg
	}
}

func TestConsumerRuntimeActorFailsWhenBootstrapTopologyIsMissing(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	failures := make(chan consumerRuntimeFailedMessage, 1)
	parent := engine.Spawn(func() actor.Receiver {
		return &runtimeProbeActor{
			cfg: ConsumerRuntimeConfig{
				AppConfig:         settings.AppConfig{},
				Generation:        7,
				Bootstrap:         runtimebootstrap.ActiveIngestionBootstrap{},
				DataPlaneRegistry: adapternats.DefaultDataPlaneRegistry(),
				Source:            "consumer.dataplane",
			},
			failed: failures,
		}
	}, "consumer-runtime-probe")
	defer engine.Poison(parent)

	select {
	case msg := <-failures:
		if msg.Generation != 7 {
			t.Fatalf("expected generation 7 failure, got %d", msg.Generation)
		}
		if msg.Err == nil || !strings.Contains(msg.Err.Error(), "bootstrap topology is unavailable") {
			t.Fatalf("expected missing topology failure, got %v", msg.Err)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("runtime failure did not arrive")
	}
}
