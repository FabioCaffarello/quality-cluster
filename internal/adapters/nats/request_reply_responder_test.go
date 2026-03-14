package nats

import (
	"context"
	"testing"

	"internal/application/configctl/contracts"
	"internal/shared/problem"
)

func TestTypedControlRouteHandlesCreateDraft(t *testing.T) {
	t.Parallel()

	registry := DefaultConfigctlRegistry()
	route := NewTypedControlRoute(registry.CreateDraft, "configctl", func(_ context.Context, command contracts.CreateDraftCommand) (contracts.CreateDraftReply, *problem.Problem) {
		if command.Name != "core" {
			t.Fatalf("expected command name %q, got %q", "core", command.Name)
		}
		return contracts.CreateDraftReply{
			Config: contracts.ConfigVersionDetail{ID: "cfg-123"},
		}, nil
	})

	replyBytes, err := route.Handler(context.Background(), mustEncodeRequest(t, registry.CreateDraft, contracts.CreateDraftCommand{
		Name:    "core",
		Format:  "json",
		Content: "{}",
	}))
	if err != nil {
		t.Fatalf("route handler: %v", err)
	}

	reply, prob := decodeControlReply[contracts.CreateDraftReply](registry.CreateDraft, replyBytes)
	if prob != nil {
		t.Fatalf("decode reply: %v", prob)
	}
	if reply.Config.ID != "cfg-123" {
		t.Fatalf("expected config id %q, got %q", "cfg-123", reply.Config.ID)
	}
}

func TestTypedControlRouteEncodesProblems(t *testing.T) {
	t.Parallel()

	registry := DefaultConfigctlRegistry()
	route := NewTypedControlRoute(registry.ValidateDraft, "configctl", func(context.Context, contracts.ValidateDraftCommand) (contracts.ValidateDraftReply, *problem.Problem) {
		return contracts.ValidateDraftReply{}, problemUnavailable()
	})

	replyBytes, err := route.Handler(context.Background(), mustEncodeRequest(t, registry.ValidateDraft, contracts.ValidateDraftCommand{
		Format:  "json",
		Content: "{}",
	}))
	if err != nil {
		t.Fatalf("route handler: %v", err)
	}

	_, prob := decodeControlReply[contracts.ValidateDraftReply](registry.ValidateDraft, replyBytes)
	if prob == nil {
		t.Fatal("expected problem")
	}
}
