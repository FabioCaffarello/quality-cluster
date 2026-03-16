package nats

import (
	"context"
	"testing"

	"internal/application/configctl/contracts"
	sharedruntime "internal/application/runtimecontracts"
	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/shared/requestctx"
)

type requestReplyClientSpy struct {
	subject string
	payload []byte
	reply   []byte
	err     error
}

func (s *requestReplyClientSpy) Request(_ context.Context, subject string, payload []byte) ([]byte, error) {
	s.subject = subject
	s.payload = payload
	return s.reply, s.err
}

func TestConfigctlGatewayCreateDraft(t *testing.T) {
	t.Parallel()

	registry := DefaultConfigctlRegistry()
	replyBytes, prob := encodeControlReply(
		registry.CreateDraft,
		"configctl",
		mustDecodeRequest[contracts.CreateDraftCommand](t, registry.CreateDraft, mustEncodeRequest(t, registry.CreateDraft, contracts.CreateDraftCommand{
			Name:    "core",
			Format:  "json",
			Content: "{}",
		})),
		contracts.CreateDraftReply{
			Config: contracts.ConfigVersionDetail{ID: "cfg-123"},
		},
		nil,
	)
	if prob != nil {
		t.Fatalf("encode reply: %v", prob)
	}

	client := &requestReplyClientSpy{reply: replyBytes}
	gateway := NewConfigctlGateway(client, "server.http")

	reply, errProb := gateway.CreateDraft(requestctx.WithCorrelationID(context.Background(), "corr-123"), contracts.CreateDraftCommand{
		Name:    "core",
		Format:  "json",
		Content: "{}",
	})
	if errProb != nil {
		t.Fatalf("expected no problem, got %v", errProb)
	}

	if client.subject != registry.CreateDraft.Subject {
		t.Fatalf("expected subject %q, got %q", registry.CreateDraft.Subject, client.subject)
	}
	if reply.Config.ID != "cfg-123" {
		t.Fatalf("expected config id %q, got %q", "cfg-123", reply.Config.ID)
	}

	request := mustDecodeRequest[contracts.CreateDraftCommand](t, registry.CreateDraft, client.payload)
	if request.CorrelationID != "corr-123" {
		t.Fatalf("expected correlation id %q, got %q", "corr-123", request.CorrelationID)
	}
}

func TestConfigctlGatewayReturnsRemoteProblem(t *testing.T) {
	t.Parallel()

	registry := DefaultConfigctlRegistry()
	replyBytes, err := encodeControlReply(
		registry.ValidateDraft,
		"configctl",
		mustDecodeRequest[contracts.ValidateDraftCommand](t, registry.ValidateDraft, mustEncodeRequest(t, registry.ValidateDraft, contracts.ValidateDraftCommand{
			Format:  "json",
			Content: "{}",
		})),
		contracts.ValidateDraftReply{},
		problemUnavailable(),
	)
	if err != nil {
		t.Fatalf("encode reply: %v", err)
	}

	_, prob := NewConfigctlGateway(&requestReplyClientSpy{reply: replyBytes}, "server.http").ValidateDraft(context.Background(), contracts.ValidateDraftCommand{
		Format:  "json",
		Content: "{}",
	})
	if prob == nil {
		t.Fatal("expected problem")
	}
}

func TestConfigctlGatewayLifecycleMethods(t *testing.T) {
	t.Parallel()

	registry := DefaultConfigctlRegistry()
	request := mustDecodeRequest[contracts.ValidateConfigCommand](t, registry.ValidateConfig, mustEncodeRequest(t, registry.ValidateConfig, contracts.ValidateConfigCommand{
		VersionID: "cfg-123",
	}))
	replyBytes, err := encodeControlReply(
		registry.ValidateConfig,
		"configctl",
		request,
		contracts.ValidateConfigReply{Valid: true},
		nil,
	)
	if err != nil {
		t.Fatalf("encode validate reply: %v", err)
	}

	validateClient := &requestReplyClientSpy{reply: replyBytes}
	validateReply, prob := NewConfigctlGateway(validateClient, "server.http").ValidateConfig(context.Background(), contracts.ValidateConfigCommand{VersionID: "cfg-123"})
	if prob != nil {
		t.Fatalf("validate config: %v", prob)
	}
	if validateClient.subject != registry.ValidateConfig.Subject || !validateReply.Valid {
		t.Fatalf("unexpected validate config gateway result")
	}

	compileRequest := mustDecodeRequest[contracts.CompileConfigCommand](t, registry.CompileConfig, mustEncodeRequest(t, registry.CompileConfig, contracts.CompileConfigCommand{
		VersionID: "cfg-123",
	}))
	replyBytes, err = encodeControlReply(
		registry.CompileConfig,
		"configctl",
		compileRequest,
		contracts.CompileConfigReply{Config: contracts.ConfigVersionDetail{ID: "cfg-123", Lifecycle: "compiled"}},
		nil,
	)
	if err != nil {
		t.Fatalf("encode compile reply: %v", err)
	}

	compileClient := &requestReplyClientSpy{reply: replyBytes}
	compileReply, prob := NewConfigctlGateway(compileClient, "server.http").CompileConfig(context.Background(), contracts.CompileConfigCommand{VersionID: "cfg-123"})
	if prob != nil {
		t.Fatalf("compile config: %v", prob)
	}
	if compileClient.subject != registry.CompileConfig.Subject || compileReply.Config.Lifecycle != "compiled" {
		t.Fatalf("unexpected compile config gateway result")
	}

	activateRequest := mustDecodeRequest[contracts.ActivateConfigCommand](t, registry.ActivateConfig, mustEncodeRequest(t, registry.ActivateConfig, contracts.ActivateConfigCommand{
		VersionID: "cfg-123",
	}))
	replyBytes, err = encodeControlReply(
		registry.ActivateConfig,
		"configctl",
		activateRequest,
		contracts.ActivateConfigReply{Config: contracts.ConfigVersionDetail{ID: "cfg-123", Lifecycle: "active"}},
		nil,
	)
	if err != nil {
		t.Fatalf("encode activate reply: %v", err)
	}

	activateClient := &requestReplyClientSpy{reply: replyBytes}
	activateReply, prob := NewConfigctlGateway(activateClient, "server.http").ActivateConfig(context.Background(), contracts.ActivateConfigCommand{VersionID: "cfg-123"})
	if prob != nil {
		t.Fatalf("activate config: %v", prob)
	}
	if activateClient.subject != registry.ActivateConfig.Subject || activateReply.Config.Lifecycle != "active" {
		t.Fatalf("unexpected activate config gateway result")
	}

	ingestionRequest := mustDecodeRequest[contracts.ListActiveIngestionBindingsQuery](t, registry.ListActiveIngestionBindings, mustEncodeRequest(t, registry.ListActiveIngestionBindings, contracts.ListActiveIngestionBindingsQuery{
		ScopeKind: "tenant",
		ScopeKey:  "br",
	}))
	replyBytes, err = encodeControlReply(
		registry.ListActiveIngestionBindings,
		"configctl",
		ingestionRequest,
		contracts.ListActiveIngestionBindingsReply{
			Bindings: []contracts.ActiveIngestionBindingRecord{{
				Binding: contracts.BindingRecord{Name: "orders", Topic: "orders.v1"},
				Fields:  []contracts.FieldRecord{{Name: "order_id", Type: "string", Required: true}},
				Runtime: sharedruntime.RuntimeRecord{
					Scope: sharedruntime.ScopeRecord{Kind: "tenant", Key: "br"},
					Config: sharedruntime.ConfigRecord{
						SetID:     "set-1",
						Key:       "core",
						VersionID: "cfg-123",
						Version:   1,
					},
				},
			}},
		},
		nil,
	)
	if err != nil {
		t.Fatalf("encode ingestion bindings reply: %v", err)
	}

	ingestionClient := &requestReplyClientSpy{reply: replyBytes}
	ingestionReply, prob := NewConfigctlGateway(ingestionClient, "server.http").ListActiveIngestionBindings(context.Background(), contracts.ListActiveIngestionBindingsQuery{
		ScopeKind: "tenant",
		ScopeKey:  "br",
	})
	if prob != nil {
		t.Fatalf("list active ingestion bindings: %v", prob)
	}
	if ingestionClient.subject != registry.ListActiveIngestionBindings.Subject || len(ingestionReply.Bindings) != 1 {
		t.Fatalf("unexpected ingestion bindings gateway result")
	}
	if len(ingestionReply.Bindings[0].Fields) != 1 {
		t.Fatalf("expected bootstrap fields to round-trip, got %+v", ingestionReply.Bindings[0])
	}

	runtimeProjectionRequest := mustDecodeRequest[contracts.ListActiveRuntimeProjectionsQuery](t, registry.ListActiveRuntimeProjections, mustEncodeRequest(t, registry.ListActiveRuntimeProjections, contracts.ListActiveRuntimeProjectionsQuery{
		ScopeKind: "tenant",
		ScopeKey:  "br",
	}))
	replyBytes, err = encodeControlReply(
		registry.ListActiveRuntimeProjections,
		"configctl",
		runtimeProjectionRequest,
		contracts.ListActiveRuntimeProjectionsReply{
			Runtimes: []contracts.RuntimeProjectionRecord{{
				Scope:       contracts.ActivationScopeRecord{Kind: "tenant", Key: "br"},
				ConfigSetID: "set-1",
				ConfigKey:   "core",
				VersionID:   "cfg-123",
				Version:     1,
				Artifact: contracts.CompilationArtifactRecord{
					ID:            "artifact-1",
					SchemaVersion: "runtime/v1",
					Checksum:      "checksum-1",
					StorageRef:    "memory://artifacts/core/v1",
					RuntimeLoader: "validator:v1",
				},
			}},
		},
		nil,
	)
	if err != nil {
		t.Fatalf("encode runtime projections reply: %v", err)
	}

	runtimeProjectionClient := &requestReplyClientSpy{reply: replyBytes}
	runtimeProjectionReply, prob := NewConfigctlGateway(runtimeProjectionClient, "validator.bootstrap").ListActiveRuntimeProjections(context.Background(), contracts.ListActiveRuntimeProjectionsQuery{
		ScopeKind: "tenant",
		ScopeKey:  "br",
	})
	if prob != nil {
		t.Fatalf("list active runtime projections: %v", prob)
	}
	if runtimeProjectionClient.subject != registry.ListActiveRuntimeProjections.Subject || len(runtimeProjectionReply.Runtimes) != 1 {
		t.Fatalf("unexpected runtime projections gateway result")
	}
}

func TestValidatorRuntimeGatewayGetActiveRuntime(t *testing.T) {
	t.Parallel()

	registry := DefaultValidatorRuntimeRegistry()
	request := mustDecodeRequest[runtimecontracts.GetActiveRuntimeQuery](t, registry.GetActive, mustEncodeRequest(t, registry.GetActive, runtimecontracts.GetActiveRuntimeQuery{}))
	replyBytes, err := encodeControlReply(
		registry.GetActive,
		"validator.runtime",
		request,
		runtimecontracts.GetActiveRuntimeReply{
			Runtime: runtimecontracts.ActiveRuntimeRecord{
				RuntimeRecord: sharedruntime.RuntimeRecord{
					Config: sharedruntime.ConfigRecord{
						Key:       "core",
						VersionID: "cfg-123",
					},
				},
			},
		},
		nil,
	)
	if err != nil {
		t.Fatalf("encode runtime reply: %v", err)
	}

	client := &requestReplyClientSpy{reply: replyBytes}
	reply, prob := NewValidatorRuntimeGateway(client, "server.http").GetActiveRuntime(context.Background(), runtimecontracts.GetActiveRuntimeQuery{})
	if prob != nil {
		t.Fatalf("get active runtime: %v", prob)
	}
	if client.subject != registry.GetActive.Subject || reply.Runtime.Config.VersionID != "cfg-123" {
		t.Fatalf("unexpected runtime gateway result")
	}
}
