package consumer

import (
	"context"
	"log/slog"

	"internal/shared/settings"

	"github.com/anthdm/hollywood/actor"
)

type bootstrapActorConfig struct {
	appConfig     settings.AppConfig
	loadBootstrap bootstrapLoaderFunc
}

type bootstrapActor struct {
	cfg    bootstrapActorConfig
	logger *slog.Logger
	cancel context.CancelFunc
}

func newBootstrapActor(cfg bootstrapActorConfig) actor.Producer {
	return func() actor.Receiver {
		return &bootstrapActor{
			cfg:    cfg,
			logger: slog.Default(),
		}
	}
}

func (a *bootstrapActor) Receive(c *actor.Context) {
	switch c.Message().(type) {
	case actor.Started:
		ctx, cancel := context.WithCancel(context.Background())
		a.cancel = cancel
		parent := c.Parent()
		engine := c.Engine()
		logger := a.logger
		go func() {
			bootstrap, prob := a.cfg.loadBootstrap(ctx, logger, a.cfg.appConfig, "consumer")
			if prob != nil {
				engine.Send(parent, activeIngestionBootstrapFailedMessage{Prob: prob})
				return
			}
			engine.Send(parent, activeIngestionBootstrapLoadedMessage{Bootstrap: bootstrap})
		}()
	case actor.Stopped:
		if a.cancel != nil {
			a.cancel()
		}
	}
}
