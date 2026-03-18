package validator

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	actorcommon "internal/actors/common"
	configctlcontracts "internal/application/configctl/contracts"
	sharedruntime "internal/application/runtimecontracts"
	runtimecontracts "internal/application/validatorruntime/contracts"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"

	"github.com/anthdm/hollywood/actor"
)

type applyRuntimeUpdateMessage struct {
	Event configdomain.ConfigActivatedEvent
}

type bootstrapRuntimeProjectionMessage struct {
	Projection configdomain.RuntimeProjection
}

type evictRuntimeScopeMessage struct {
	Scope         configdomain.ActivationScope
	ConfigSetID   string
	VersionID     string
	DeactivatedAt time.Time
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
		a.applyProjection(msg.Event.Projection)
	case bootstrapRuntimeProjectionMessage:
		a.applyProjection(msg.Projection)
	case evictRuntimeScopeMessage:
		a.evictScope(msg)
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
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
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

func (a *RuntimeCacheActor) applyProjection(projection configdomain.RuntimeProjection) {
	scope := projection.Scope.Normalize().String()
	if current, ok := a.runtimes[scope]; ok && current.projection.ActivatedAt.After(projection.ActivatedAt) {
		return
	}
	a.runtimes[scope] = cachedRuntime{
		projection: projection,
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
}

func (a *RuntimeCacheActor) evictScope(msg evictRuntimeScopeMessage) {
	scope := msg.Scope.Normalize()
	current, ok := a.runtimes[scope.String()]
	if !ok {
		return
	}
	if msg.ConfigSetID != "" && msg.VersionID != "" {
		// Ignore stale deactivation events that belong to an older runtime.
		if current.projection.ConfigSetID != msg.ConfigSetID || current.projection.VersionID != msg.VersionID {
			return
		}
	}
	if !msg.DeactivatedAt.IsZero() && current.projection.ActivatedAt.After(msg.DeactivatedAt.UTC()) {
		return
	}
	delete(a.runtimes, scope.String())
	a.logger.Info("validator runtime cache cleared", "scope", scope.String())
}

func snapshotRuntime(runtime cachedRuntime) runtimecontracts.ActiveRuntimeRecord {
	return runtimecontracts.ActiveRuntimeRecord{
		RuntimeRecord: sharedruntime.RecordFromProjection(runtime.projection),
		LoadedAt:      runtime.loadedAt,
	}
}

func runtimeProjectionFromRecord(record configctlcontracts.RuntimeProjectionRecord) configdomain.RuntimeProjection {
	projection := configdomain.RuntimeProjection{
		Scope: configdomain.ActivationScope{
			Kind: record.Scope.Kind,
			Key:  record.Scope.Key,
		}.Normalize(),
		ConfigSetID: record.ConfigSetID,
		ConfigKey:   record.ConfigKey,
		VersionID:   record.VersionID,
		Version:     record.Version,
		Artifact: configdomain.CompilationArtifact{
			ID:              record.Artifact.ID,
			SchemaVersion:   record.Artifact.SchemaVersion,
			Checksum:        record.Artifact.Checksum,
			StorageRef:      record.Artifact.StorageRef,
			RuntimeLoader:   record.Artifact.RuntimeLoader,
			Capabilities:    append([]string(nil), record.Artifact.Capabilities...),
			CompilerVersion: record.Artifact.CompilerVersion,
			CreatedAt:       record.Artifact.CreatedAt,
		},
		ActivatedAt:        record.ActivatedAt,
		DefinitionChecksum: record.DefinitionChecksum,
	}
	for _, binding := range record.Bindings {
		projection.Bindings = append(projection.Bindings, configdomain.Binding{
			Name:  binding.Name,
			Topic: binding.Topic,
		})
	}
	for _, field := range record.Fields {
		projection.Fields = append(projection.Fields, configdomain.Field{
			Name:     field.Name,
			Type:     configdomain.FieldType(field.Type),
			Required: field.Required,
		})
	}
	for _, rule := range record.Rules {
		projection.Rules = append(projection.Rules, configdomain.Rule{
			Name:          rule.Name,
			Field:         rule.Field,
			Operator:      configdomain.RuleOperator(rule.Operator),
			ExpectedValue: rule.ExpectedValue,
			Severity:      configdomain.RuleSeverity(rule.Severity),
		})
	}
	return projection
}

func (a *RuntimeCacheActor) reply(c *actor.Context, msg any) {
	if sender := c.Sender(); sender != nil {
		c.Send(sender, msg)
	}
}
