package nats

import (
	"context"
	"testing"

	"internal/application/configctl/contracts"
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
			Config: contracts.ConfigRecord{ID: "cfg-123"},
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
