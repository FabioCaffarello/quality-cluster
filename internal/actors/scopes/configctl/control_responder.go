package configctl

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	actorcommon "internal/actors/common"
	adapternats "internal/adapters/nats"
	"internal/application/configctl/contracts"
	"internal/shared/problem"
	"internal/shared/requestctx"

	"github.com/anthdm/hollywood/actor"
)

type ControlResponderConfig struct {
	URL            string
	Source         string
	ControlRouter  *actor.PID
	Registry       adapternats.ConfigctlRegistry
	RequestTimeout time.Duration
}

type ControlResponderActor struct {
	cfg       ControlResponderConfig
	logger    *slog.Logger
	engine    *actor.Engine
	responder *adapternats.RequestReplyResponder
}

func NewControlResponderActor(cfg ControlResponderConfig) actor.Producer {
	return func() actor.Receiver {
		return &ControlResponderActor{cfg: cfg}
	}
}

func (a *ControlResponderActor) Receive(c *actor.Context) {
	if a.logger == nil {
		a.logger = slog.Default()
	}
	if a.engine == nil && c != nil {
		a.engine = c.Engine()
	}

	switch msg := c.Message().(type) {
	case actor.Started:
		routes := []adapternats.ControlRoute{
			adapternats.NewTypedControlRoute(a.cfg.Registry.CreateDraft, a.cfg.Source, a.handleCreateDraft),
			adapternats.NewTypedControlRoute(a.cfg.Registry.GetConfig, a.cfg.Source, a.handleGetConfig),
			adapternats.NewTypedControlRoute(a.cfg.Registry.GetActive, a.cfg.Source, a.handleGetActive),
			adapternats.NewTypedControlRoute(a.cfg.Registry.ListActiveRuntimeProjections, a.cfg.Source, a.handleListActiveRuntimeProjections),
			adapternats.NewTypedControlRoute(a.cfg.Registry.ListActiveIngestionBindings, a.cfg.Source, a.handleListActiveIngestionBindings),
			adapternats.NewTypedControlRoute(a.cfg.Registry.ListConfigs, a.cfg.Source, a.handleListConfigs),
			adapternats.NewTypedControlRoute(a.cfg.Registry.ValidateDraft, a.cfg.Source, a.handleValidateDraft),
			adapternats.NewTypedControlRoute(a.cfg.Registry.ValidateConfig, a.cfg.Source, a.handleValidateConfig),
			adapternats.NewTypedControlRoute(a.cfg.Registry.CompileConfig, a.cfg.Source, a.handleCompileConfig),
			adapternats.NewTypedControlRoute(a.cfg.Registry.ActivateConfig, a.cfg.Source, a.handleActivateConfig),
		}
		responder := adapternats.NewRequestReplyResponder(a.cfg.URL, routes)
		if err := responder.Start(); err != nil {
			a.logger.Error("start control responder", "error", err)
			c.Engine().Poison(c.PID())
			return
		}
		a.responder = responder
	case actor.Stopped:
		if a.responder != nil {
			if err := a.responder.Close(); err != nil {
				a.logger.Error("close control responder", "error", err)
			}
		}
	default:
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
		a.logger.Warn("configctl control responder: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (a *ControlResponderActor) handleCreateDraft(ctx context.Context, command contracts.CreateDraftCommand) (contracts.CreateDraftReply, *problem.Problem) {
	result, prob := requestActor[createDraftResult](a.engine, a.cfg.ControlRouter, createDraftMessage{
		Command:       command,
		CorrelationID: requestctx.CorrelationID(ctx),
	}, a.cfg.RequestTimeout)
	return result.Reply, mergeProblems(result.Prob, prob)
}

func (a *ControlResponderActor) handleGetConfig(ctx context.Context, query contracts.GetConfigQuery) (contracts.GetConfigReply, *problem.Problem) {
	result, prob := requestActor[getConfigResult](a.engine, a.cfg.ControlRouter, getConfigMessage{
		Query:         query,
		CorrelationID: requestctx.CorrelationID(ctx),
	}, a.cfg.RequestTimeout)
	return result.Reply, mergeProblems(result.Prob, prob)
}

func (a *ControlResponderActor) handleGetActive(ctx context.Context, query contracts.GetActiveConfigQuery) (contracts.GetActiveConfigReply, *problem.Problem) {
	result, prob := requestActor[getActiveConfigResult](a.engine, a.cfg.ControlRouter, getActiveConfigMessage{
		Query:         query,
		CorrelationID: requestctx.CorrelationID(ctx),
	}, a.cfg.RequestTimeout)
	return result.Reply, mergeProblems(result.Prob, prob)
}

func (a *ControlResponderActor) handleListActiveRuntimeProjections(ctx context.Context, query contracts.ListActiveRuntimeProjectionsQuery) (contracts.ListActiveRuntimeProjectionsReply, *problem.Problem) {
	result, prob := requestActor[listActiveRuntimeProjectionsResult](a.engine, a.cfg.ControlRouter, listActiveRuntimeProjectionsMessage{
		Query:         query,
		CorrelationID: requestctx.CorrelationID(ctx),
	}, a.cfg.RequestTimeout)
	return result.Reply, mergeProblems(result.Prob, prob)
}

func (a *ControlResponderActor) handleListConfigs(ctx context.Context, query contracts.ListConfigsQuery) (contracts.ListConfigsReply, *problem.Problem) {
	result, prob := requestActor[listConfigsResult](a.engine, a.cfg.ControlRouter, listConfigsMessage{
		Query:         query,
		CorrelationID: requestctx.CorrelationID(ctx),
	}, a.cfg.RequestTimeout)
	return result.Reply, mergeProblems(result.Prob, prob)
}

func (a *ControlResponderActor) handleListActiveIngestionBindings(ctx context.Context, query contracts.ListActiveIngestionBindingsQuery) (contracts.ListActiveIngestionBindingsReply, *problem.Problem) {
	result, prob := requestActor[listActiveIngestionBindingsResult](a.engine, a.cfg.ControlRouter, listActiveIngestionBindingsMessage{
		Query:         query,
		CorrelationID: requestctx.CorrelationID(ctx),
	}, a.cfg.RequestTimeout)
	return result.Reply, mergeProblems(result.Prob, prob)
}

func (a *ControlResponderActor) handleValidateDraft(ctx context.Context, command contracts.ValidateDraftCommand) (contracts.ValidateDraftReply, *problem.Problem) {
	result, prob := requestActor[validateDraftResult](a.engine, a.cfg.ControlRouter, validateDraftMessage{
		Command:       command,
		CorrelationID: requestctx.CorrelationID(ctx),
	}, a.cfg.RequestTimeout)
	return result.Reply, mergeProblems(result.Prob, prob)
}

func (a *ControlResponderActor) handleValidateConfig(ctx context.Context, command contracts.ValidateConfigCommand) (contracts.ValidateConfigReply, *problem.Problem) {
	result, prob := requestActor[validateConfigResult](a.engine, a.cfg.ControlRouter, validateConfigMessage{
		Command:       command,
		CorrelationID: requestctx.CorrelationID(ctx),
	}, a.cfg.RequestTimeout)
	return result.Reply, mergeProblems(result.Prob, prob)
}

func (a *ControlResponderActor) handleCompileConfig(ctx context.Context, command contracts.CompileConfigCommand) (contracts.CompileConfigReply, *problem.Problem) {
	result, prob := requestActor[compileConfigResult](a.engine, a.cfg.ControlRouter, compileConfigMessage{
		Command:       command,
		CorrelationID: requestctx.CorrelationID(ctx),
	}, a.cfg.RequestTimeout)
	return result.Reply, mergeProblems(result.Prob, prob)
}

func (a *ControlResponderActor) handleActivateConfig(ctx context.Context, command contracts.ActivateConfigCommand) (contracts.ActivateConfigReply, *problem.Problem) {
	result, prob := requestActor[activateConfigResult](a.engine, a.cfg.ControlRouter, activateConfigMessage{
		Command:       command,
		CorrelationID: requestctx.CorrelationID(ctx),
	}, a.cfg.RequestTimeout)
	return result.Reply, mergeProblems(result.Prob, prob)
}

func requestActor[T any](engine *actor.Engine, pid *actor.PID, message any, timeout time.Duration) (T, *problem.Problem) {
	var zero T
	if engine == nil || pid == nil {
		return zero, problem.New(problem.Unavailable, "control router is unavailable")
	}
	if timeout <= 0 {
		timeout = 5 * time.Second
	}

	response := engine.Request(pid, message, timeout)
	result, err := response.Result()
	if err != nil {
		return zero, problem.Wrap(err, problem.Unavailable, "request control router")
	}

	typed, ok := result.(T)
	if !ok {
		return zero, problem.New(problem.Internal, "control router response is invalid")
	}

	return typed, nil
}

func mergeProblems(primary, secondary *problem.Problem) *problem.Problem {
	if primary != nil {
		return primary
	}
	return secondary
}
