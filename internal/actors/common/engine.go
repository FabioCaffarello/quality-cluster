package actorcommon

import (
	"github.com/anthdm/hollywood/actor"
)

func NewDefaultEngine() (*actor.Engine, error) {
	return actor.NewEngine(actor.NewEngineConfig())
}

func NewDeafultEngine() (*actor.Engine, error) {
	return NewDefaultEngine()
}
