package actorserver

import (
	"log/slog"
	"reflect"
	"github.com/anthdm/hollywood/actor"
)

type ServerRouter struct {
	ctx        *actor.Context
	quitch     chan struct{}
}

func NewServerRouter()actor.Producer {
	return func() actor.Receiver {
		return &ServerRouter{
			quitch:     make(chan struct{}),
		}
	}
}


func (sr *ServerRouter) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		sr.ctx = c
		// TODO: consumer

	// broadcast event envelope to nats stream
	case actor.Stopped:
		close(sr.quitch)
	default:
		slog.Error("unknown message", "msg", msg, "type", reflect.TypeOf(msg))
	}
}