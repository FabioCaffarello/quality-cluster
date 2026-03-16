package consumer

import (
	"context"
	"log/slog"
	"strings"
	"time"

	adapternats "internal/adapters/nats"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"
	"internal/shared/settings"

	"github.com/anthdm/hollywood/actor"
)

type bootstrapActorConfig struct {
	appConfig          settings.AppConfig
	loadBootstrap      bootstrapLoaderFunc
	runtimeChangedSpec adapternats.ConsumerSpec
	reconcileInterval  time.Duration
}

type bootstrapActor struct {
	cfg                    bootstrapActorConfig
	logger                 *slog.Logger
	cancel                 context.CancelFunc
	lastSignature          string
	bootstrapped           bool
	runtimeChangedConsumer *adapternats.IngestionRuntimeChangedConsumer
	refreshCh              chan struct{}
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
	switch msg := c.Message().(type) {
	case actor.Started:
		ctx, cancel := context.WithCancel(context.Background())
		a.cancel = cancel
		a.refreshCh = make(chan struct{}, 1)
		parent := c.Parent()
		engine := c.Engine()
		logger := a.logger
		if strings.TrimSpace(a.cfg.appConfig.NATS.URL) != "" {
			consumer := adapternats.NewIngestionRuntimeChangedConsumer(a.cfg.appConfig.NATS.URL, a.cfg.runtimeChangedSpec, &bootstrapRefreshNotifier{signals: a.refreshCh})
			if err := consumer.Start(); err != nil {
				engine.Send(parent, activeIngestionBootstrapFailedMessage{Prob: problemFromError("start consumer runtime refresh consumer", err)})
				return
			}
			a.runtimeChangedConsumer = consumer
		}
		go a.refreshLoop(ctx, logger, engine, parent)
	case refreshActiveIngestionBootstrapMessage:
		if a.refreshCh != nil && shouldRefreshForEvent(msg.Event) {
			select {
			case a.refreshCh <- struct{}{}:
			default:
			}
		}
	case actor.Stopped:
		if a.cancel != nil {
			a.cancel()
		}
		if a.runtimeChangedConsumer != nil {
			_ = a.runtimeChangedConsumer.Close()
		}
	}
}

func (a *bootstrapActor) refreshLoop(ctx context.Context, logger *slog.Logger, engine *actor.Engine, parent *actor.PID) {
	a.refreshBootstrap(ctx, logger, engine, parent)

	var reconcileTicker *time.Ticker
	if interval := a.reconcileInterval(); interval > 0 {
		reconcileTicker = time.NewTicker(interval)
		defer reconcileTicker.Stop()
	}

	var reconcileTick <-chan time.Time
	if reconcileTicker != nil {
		reconcileTick = reconcileTicker.C
	}

	for {
		select {
		case <-ctx.Done():
			return
		case <-a.refreshCh:
			a.refreshBootstrap(ctx, logger, engine, parent)
		case <-reconcileTick:
			a.refreshBootstrap(ctx, logger, engine, parent)
		}
	}
}

func (a *bootstrapActor) reconcileInterval() time.Duration {
	if a.cfg.reconcileInterval > 0 {
		return a.cfg.reconcileInterval
	}
	return a.cfg.appConfig.Bootstrap.ReconcileIntervalDuration()
}

func (a *bootstrapActor) refreshBootstrap(ctx context.Context, logger *slog.Logger, engine *actor.Engine, parent *actor.PID) {
	bootstrap, prob := a.cfg.loadBootstrap(ctx, logger, a.cfg.appConfig, "consumer")
	if prob != nil {
		if !a.bootstrapped {
			engine.Send(parent, activeIngestionBootstrapFailedMessage{Prob: prob})
			return
		}
		if logger != nil {
			logger.Warn("refresh consumer bootstrap", "error", prob)
		}
		return
	}

	signature := bootstrap.Signature()
	if !a.bootstrapped || signature != a.lastSignature {
		a.lastSignature = signature
		a.bootstrapped = true
		engine.Send(parent, activeIngestionBootstrapLoadedMessage{Bootstrap: bootstrap})
	}
}

type bootstrapRefreshNotifier struct {
	signals chan<- struct{}
}

func (n *bootstrapRefreshNotifier) HandleIngestionRuntimeChanged(_ context.Context, _ configdomain.IngestionRuntimeChangedEvent) *problem.Problem {
	if n == nil || n.signals == nil {
		return problemFromMessage("bootstrap refresh channel is unavailable")
	}
	select {
	case n.signals <- struct{}{}:
	default:
	}
	return nil
}

func shouldRefreshForEvent(event configdomain.IngestionRuntimeChangedEvent) bool {
	switch event.ChangeType {
	case configdomain.IngestionRuntimeChangeActivated, configdomain.IngestionRuntimeChangeCleared:
		return true
	default:
		return true
	}
}

func problemFromError(msg string, err error) *problem.Problem {
	return problem.Wrap(err, problem.Unavailable, msg)
}

func problemFromMessage(msg string) *problem.Problem {
	return problem.New(problem.Unavailable, msg)
}
