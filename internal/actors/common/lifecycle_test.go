package actorcommon

import (
	"testing"

	"github.com/anthdm/hollywood/actor"
)

func TestShouldIgnoreLifecycleMessage(t *testing.T) {
	t.Parallel()

	if !ShouldIgnoreLifecycleMessage(actor.Initialized{}) {
		t.Fatal("expected actor.Initialized to be ignored")
	}
	if !ShouldIgnoreLifecycleMessage(actor.Started{}) {
		t.Fatal("expected actor.Started to be ignored")
	}
	if ShouldIgnoreLifecycleMessage(actor.Stopped{}) {
		t.Fatal("expected actor.Stopped to remain explicit in actor switches")
	}
	if ShouldIgnoreLifecycleMessage(struct{}{}) {
		t.Fatal("expected arbitrary messages not to be ignored")
	}
}
