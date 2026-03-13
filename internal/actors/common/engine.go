package actorcommon

import (
	"github.com/anthdm/hollywood/actor"
)

func NewDeafultEngine() (*actor.Engine,  error) {
	return actor.NewEngine(actor.NewEngineConfig())
}