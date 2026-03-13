package configctl

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	configapp "internal/application/configctl"
	"internal/application/configctl/contracts"
	"internal/shared/problem"
	"internal/shared/requestctx"

	"github.com/anthdm/hollywood/actor"
)

type ControlRouterConfig struct {
	EventRouterPID *actor.PID
	RequestTimeout time.Duration
}

type ControlRouterActor struct {
	cfg           ControlRouterConfig
	logger        *slog.Logger
	engine        *actor.Engine
	repository    *runtimeStore
	createDraft   *configapp.CreateDraftUseCase
	getConfig     *configapp.GetConfigUseCase
	getActive     *configapp.GetActiveConfigUseCase
	listConfigs   *configapp.ListConfigsUseCase
	validateDraft *configapp.ValidateDraftUseCase
}

func NewControlRouterActor(cfg ControlRouterConfig) actor.Producer {
	return func() actor.Receiver {
		return &ControlRouterActor{cfg: cfg}
	}
}

func (a *ControlRouterActor) Receive(c *actor.Context) {
	a.ensureDefaults(c)

	switch msg := c.Message().(type) {
	case actor.Started:
		a.logger.Info("configctl control router started")
	case createDraftMessage:
		reply, prob := a.createDraft.Execute(requestctx.WithCorrelationID(context.Background(), msg.CorrelationID), msg.Command)
		a.reply(c, createDraftResult{Reply: reply, Prob: prob})
	case getConfigMessage:
		reply, prob := a.getConfig.Execute(requestctx.WithCorrelationID(context.Background(), msg.CorrelationID), msg.Query)
		a.reply(c, getConfigResult{Reply: reply, Prob: prob})
	case getActiveConfigMessage:
		reply, prob := a.getActive.Execute(requestctx.WithCorrelationID(context.Background(), msg.CorrelationID), msg.Query)
		a.reply(c, getActiveConfigResult{Reply: reply, Prob: prob})
	case listConfigsMessage:
		reply, prob := a.listConfigs.Execute(requestctx.WithCorrelationID(context.Background(), msg.CorrelationID), msg.Query)
		a.reply(c, listConfigsResult{Reply: reply, Prob: prob})
	case validateDraftMessage:
		reply, prob := a.validateDraft.Execute(requestctx.WithCorrelationID(context.Background(), msg.CorrelationID), msg.Command)
		a.reply(c, validateDraftResult{Reply: reply, Prob: prob})
	default:
		a.logger.Warn("configctl control router: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (a *ControlRouterActor) ensureDefaults(c *actor.Context) {
	if a.logger == nil {
		a.logger = slog.Default()
	}
	if a.engine == nil && c != nil {
		a.engine = c.Engine()
	}
	if a.repository == nil {
		a.repository = newRuntimeStore()
	}
	if a.createDraft == nil {
		publisher := &actorRuntimeEventPublisher{
			engine:   a.engine,
			eventPID: a.cfg.EventRouterPID,
			timeout:  a.cfg.RequestTimeout,
		}
		a.createDraft = configapp.NewCreateDraftUseCase(a.repository, publisher)
	}
	if a.getConfig == nil {
		a.getConfig = configapp.NewGetConfigUseCase(a.repository)
	}
	if a.getActive == nil {
		a.getActive = configapp.NewGetActiveConfigUseCase(a.repository)
	}
	if a.listConfigs == nil {
		a.listConfigs = configapp.NewListConfigsUseCase(a.repository)
	}
	if a.validateDraft == nil {
		a.validateDraft = configapp.NewValidateDraftUseCase()
	}
}

func (a *ControlRouterActor) reply(c *actor.Context, msg any) {
	if sender := c.Sender(); sender != nil {
		c.Send(sender, msg)
	}
}

type actorRuntimeEventPublisher struct {
	engine   *actor.Engine
	eventPID *actor.PID
	timeout  time.Duration
}

func (p *actorRuntimeEventPublisher) Publish(_ context.Context, event contracts.RuntimeEvent) *problem.Problem {
	if p == nil || p.engine == nil || p.eventPID == nil {
		return problem.New(problem.Unavailable, "runtime event publisher is unavailable")
	}

	timeout := p.timeout
	if timeout <= 0 {
		timeout = 5 * time.Second
	}

	response := p.engine.Request(p.eventPID, publishRuntimeEventMessage{Event: event}, timeout)
	result, err := response.Result()
	if err != nil {
		return problem.Wrap(err, problem.Unavailable, "publish runtime event")
	}

	publishResult, ok := result.(publishRuntimeEventResult)
	if !ok {
		return problem.New(problem.Internal, "runtime event response is invalid")
	}

	return publishResult.Prob
}
