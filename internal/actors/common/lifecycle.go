package actorcommon

import "github.com/anthdm/hollywood/actor"

// ShouldIgnoreLifecycleMessage reports whether an actor message is a normal
// framework lifecycle signal that should not be logged as an unknown warning.
func ShouldIgnoreLifecycleMessage(msg any) bool {
	switch msg.(type) {
	case actor.Initialized, actor.Started:
		return true
	default:
		return false
	}
}
