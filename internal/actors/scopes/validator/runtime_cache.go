package validator

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	sharedruntime "internal/application/runtimecontracts"
	runtimecontracts "internal/application/validatorruntime/contracts"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"

	"github.com/anthdm/hollywood/actor"
)

type applyRuntimeUpdateMessage struct {
	Event configdomain.ConfigActivatedEvent
}

type queryActiveRuntimeMessage struct {
	Query         runtimecontracts.GetActiveRuntimeQuery
	CorrelationID string
}

type queryActiveRuntimeResult struct {
	Reply runtimecontracts.GetActiveRuntimeReply
	Prob  *problem.Problem
}

type resolveRuntimeProjectionMessage struct {
	ScopeKind     string
	ScopeKey      string
	CorrelationID string
}

type resolveRuntimeProjectionResult struct {
	Runtime  configdomain.RuntimeProjection
	LoadedAt time.Time
	Prob     *problem.Problem
}

type cachedRuntime struct {
	projection configdomain.RuntimeProjection
	loadedAt   time.Time
}

type RuntimeCacheActor struct {
	logger   *slog.Logger
	runtimes map[string]cachedRuntime
}

func NewRuntimeCacheActor() actor.Producer {
	return func() actor.Receiver {
		return &RuntimeCacheActor{
			logger:   slog.Default(),
			runtimes: make(map[string]cachedRuntime),
		}
	}
}

func (a *RuntimeCacheActor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		a.logger.Info("validator runtime cache started")
	case applyRuntimeUpdateMessage:
		scope := msg.Event.Projection.Scope.Normalize().String()
		a.runtimes[scope] = cachedRuntime{
			projection: msg.Event.Projection,
			loadedAt:   time.Now().UTC(),
		}
		runtime := a.runtimes[scope]
		a.logger.Info(
			"validator runtime cache updated",
			"config_set_id", runtime.projection.ConfigSetID,
			"version_id", runtime.projection.VersionID,
			"version", runtime.projection.Version,
			"scope", runtime.projection.Scope.String(),
			"artifact_checksum", runtime.projection.Artifact.Checksum,
		)
	case queryActiveRuntimeMessage:
		runtime, prob := a.runtimeForScope(configdomain.ActivationScope{
			Kind: msg.Query.ScopeKind,
			Key:  msg.Query.ScopeKey,
		})
		a.reply(c, queryActiveRuntimeResult{
			Reply: runtimecontracts.GetActiveRuntimeReply{Runtime: snapshotRuntime(runtime)},
			Prob:  prob,
		})
	case resolveRuntimeProjectionMessage:
		runtime, prob := a.runtimeForScope(configdomain.ActivationScope{
			Kind: msg.ScopeKind,
			Key:  msg.ScopeKey,
		})
		a.reply(c, resolveRuntimeProjectionResult{
			Runtime:  runtime.projection,
			LoadedAt: runtime.loadedAt,
			Prob:     prob,
		})
	default:
		a.logger.Warn("validator runtime cache: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

type cacheForwarder struct {
	engine   *actor.Engine
	cachePID *actor.PID
}

func (f *cacheForwarder) HandleConfigActivated(_ context.Context, event configdomain.ConfigActivatedEvent) *problem.Problem {
	if f == nil || f.engine == nil || f.cachePID == nil {
		return problem.New(problem.Unavailable, "validator cache is unavailable")
	}
	f.engine.Send(f.cachePID, applyRuntimeUpdateMessage{Event: event})
	return nil
}

func (a *RuntimeCacheActor) runtimeForScope(scope configdomain.ActivationScope) (cachedRuntime, *problem.Problem) {
	if a == nil || len(a.runtimes) == 0 {
		return cachedRuntime{}, problem.New(problem.NotFound, "validator runtime is not loaded").MarkRetryable()
	}

	scope = scope.Normalize()
	runtime, ok := a.runtimes[scope.String()]
	if !ok {
		return cachedRuntime{}, problem.New(problem.NotFound, "validator runtime scope is not loaded").MarkRetryable()
	}
	return runtime, nil
}

func snapshotRuntime(runtime cachedRuntime) runtimecontracts.ActiveRuntimeRecord {
	return runtimecontracts.ActiveRuntimeRecord{
		RuntimeRecord: sharedruntime.RecordFromProjection(runtime.projection),
		LoadedAt:      runtime.loadedAt,
	}
}

func (a *RuntimeCacheActor) reply(c *actor.Context, msg any) {
	if sender := c.Sender(); sender != nil {
		c.Send(sender, msg)
	}
}
