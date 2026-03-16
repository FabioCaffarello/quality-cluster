package configctl

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	actorcommon "internal/actors/common"
	memoryrepo "internal/adapters/repositories/memory/configctl"
	configapp "internal/application/configctl"
	"internal/shared/events"
	"internal/shared/problem"
	"internal/shared/requestctx"

	"github.com/anthdm/hollywood/actor"
)

type ControlRouterConfig struct {
	EventRouterPID *actor.PID
	RequestTimeout time.Duration
}

type ControlRouterActor struct {
	cfg                    ControlRouterConfig
	logger                 *slog.Logger
	engine                 *actor.Engine
	repository             configapp.Repository
	createDraft            *configapp.CreateDraftUseCase
	getConfig              *configapp.GetConfigUseCase
	getActive              *configapp.GetActiveConfigUseCase
	listRuntimeProjections *configapp.ListActiveRuntimeProjectionsUseCase
	listIngestionBindings  *configapp.ListActiveIngestionBindingsUseCase
	listConfigs            *configapp.ListConfigsUseCase
	validateDraft          *configapp.ValidateDraftUseCase
	validateConfig         *configapp.ValidateConfigUseCase
	compileConfig          *configapp.CompileConfigUseCase
	activateConfig         *configapp.ActivateConfigUseCase
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
	case listActiveRuntimeProjectionsMessage:
		reply, prob := a.listRuntimeProjections.Execute(requestctx.WithCorrelationID(context.Background(), msg.CorrelationID), msg.Query)
		a.reply(c, listActiveRuntimeProjectionsResult{Reply: reply, Prob: prob})
	case listActiveIngestionBindingsMessage:
		reply, prob := a.listIngestionBindings.Execute(requestctx.WithCorrelationID(context.Background(), msg.CorrelationID), msg.Query)
		a.reply(c, listActiveIngestionBindingsResult{Reply: reply, Prob: prob})
	case listConfigsMessage:
		reply, prob := a.listConfigs.Execute(requestctx.WithCorrelationID(context.Background(), msg.CorrelationID), msg.Query)
		a.reply(c, listConfigsResult{Reply: reply, Prob: prob})
	case validateDraftMessage:
		reply, prob := a.validateDraft.Execute(requestctx.WithCorrelationID(context.Background(), msg.CorrelationID), msg.Command)
		a.reply(c, validateDraftResult{Reply: reply, Prob: prob})
	case validateConfigMessage:
		reply, prob := a.validateConfig.Execute(requestctx.WithCorrelationID(context.Background(), msg.CorrelationID), msg.Command)
		a.reply(c, validateConfigResult{Reply: reply, Prob: prob})
	case compileConfigMessage:
		reply, prob := a.compileConfig.Execute(requestctx.WithCorrelationID(context.Background(), msg.CorrelationID), msg.Command)
		a.reply(c, compileConfigResult{Reply: reply, Prob: prob})
	case activateConfigMessage:
		reply, prob := a.activateConfig.Execute(requestctx.WithCorrelationID(context.Background(), msg.CorrelationID), msg.Command)
		a.reply(c, activateConfigResult{Reply: reply, Prob: prob})
	default:
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
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
		a.repository = memoryrepo.NewRepository(nil)
	}
	publisher := &actorDomainEventPublisher{
		engine:   a.engine,
		eventPID: a.cfg.EventRouterPID,
		timeout:  a.cfg.RequestTimeout,
	}
	if a.createDraft == nil {
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
	if a.listRuntimeProjections == nil {
		a.listRuntimeProjections = configapp.NewListActiveRuntimeProjectionsUseCase(a.repository)
	}
	if a.listIngestionBindings == nil {
		a.listIngestionBindings = configapp.NewListActiveIngestionBindingsUseCase(a.repository)
	}
	if a.validateDraft == nil {
		a.validateDraft = configapp.NewValidateDraftUseCase()
	}
	if a.validateConfig == nil {
		a.validateConfig = configapp.NewValidateConfigUseCase(a.repository, publisher)
	}
	if a.compileConfig == nil {
		a.compileConfig = configapp.NewCompileConfigUseCase(a.repository, publisher)
	}
	if a.activateConfig == nil {
		a.activateConfig = configapp.NewActivateConfigUseCase(a.repository, publisher)
	}
}

func (a *ControlRouterActor) reply(c *actor.Context, msg any) {
	if sender := c.Sender(); sender != nil {
		c.Send(sender, msg)
	}
}

type actorDomainEventPublisher struct {
	engine   *actor.Engine
	eventPID *actor.PID
	timeout  time.Duration
}

func (p *actorDomainEventPublisher) Publish(_ context.Context, event events.Event) *problem.Problem {
	if p == nil || p.engine == nil || p.eventPID == nil {
		return problem.New(problem.Unavailable, "runtime event publisher is unavailable")
	}

	timeout := p.timeout
	if timeout <= 0 {
		timeout = 5 * time.Second
	}

	response := p.engine.Request(p.eventPID, publishDomainEventMessage{Event: event}, timeout)
	result, err := response.Result()
	if err != nil {
		return problem.Wrap(err, problem.Unavailable, "publish runtime event")
	}

	publishResult, ok := result.(publishDomainEventResult)
	if !ok {
		return problem.New(problem.Internal, "runtime event response is invalid")
	}

	return publishResult.Prob
}
