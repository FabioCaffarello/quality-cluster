package configctlclient

import (
	"context"
	"testing"

	"internal/application/configctl/contracts"
	"internal/shared/problem"
)

type gatewaySpy struct {
	createDraftCommand contracts.CreateDraftCommand
	getConfigQuery     contracts.GetConfigQuery
	listCalled         bool
	validateCommand    contracts.ValidateDraftCommand

	createDraftReply contracts.CreateDraftReply
	getConfigReply   contracts.GetConfigReply
	listReply        contracts.ListConfigsReply
	validateReply    contracts.ValidateDraftReply
	prob             *problem.Problem
}

func (s *gatewaySpy) CreateDraft(_ context.Context, command contracts.CreateDraftCommand) (contracts.CreateDraftReply, *problem.Problem) {
	s.createDraftCommand = command
	return s.createDraftReply, s.prob
}

func (s *gatewaySpy) GetConfig(_ context.Context, query contracts.GetConfigQuery) (contracts.GetConfigReply, *problem.Problem) {
	s.getConfigQuery = query
	return s.getConfigReply, s.prob
}

func (s *gatewaySpy) GetActiveConfig(context.Context, contracts.GetActiveConfigQuery) (contracts.GetActiveConfigReply, *problem.Problem) {
	return contracts.GetActiveConfigReply{}, s.prob
}

func (s *gatewaySpy) ListConfigs(context.Context, contracts.ListConfigsQuery) (contracts.ListConfigsReply, *problem.Problem) {
	s.listCalled = true
	return s.listReply, s.prob
}

func (s *gatewaySpy) ValidateDraft(_ context.Context, command contracts.ValidateDraftCommand) (contracts.ValidateDraftReply, *problem.Problem) {
	s.validateCommand = command
	return s.validateReply, s.prob
}

func TestCreateDraftUseCaseCallsGateway(t *testing.T) {
	t.Parallel()

	gateway := &gatewaySpy{
		createDraftReply: contracts.CreateDraftReply{
			Config: contracts.ConfigRecord{ID: "cfg-123"},
		},
	}

	reply, prob := NewCreateDraftUseCase(gateway).Execute(context.Background(), contracts.CreateDraftCommand{
		Name:    "core",
		Format:  "json",
		Content: "{}",
	})
	if prob != nil {
		t.Fatalf("expected no problem, got %v", prob)
	}
	if reply.Config.ID != "cfg-123" {
		t.Fatalf("expected config id %q, got %q", "cfg-123", reply.Config.ID)
	}
}

func TestGetAndListUseCasesCallGateway(t *testing.T) {
	t.Parallel()

	gateway := &gatewaySpy{
		getConfigReply: contracts.GetConfigReply{
			Config: contracts.ConfigRecord{ID: "cfg-123"},
		},
		listReply: contracts.ListConfigsReply{
			Configs: []contracts.ConfigRecord{{ID: "cfg-123"}},
		},
	}

	getReply, prob := NewGetConfigUseCase(gateway).Execute(context.Background(), contracts.GetConfigQuery{ID: "cfg-123"})
	if prob != nil {
		t.Fatalf("get config: %v", prob)
	}
	if getReply.Config.ID != "cfg-123" {
		t.Fatalf("expected config id %q, got %q", "cfg-123", getReply.Config.ID)
	}

	listReply, prob := NewListConfigsUseCase(gateway).Execute(context.Background(), contracts.ListConfigsQuery{})
	if prob != nil {
		t.Fatalf("list configs: %v", prob)
	}
	if !gateway.listCalled || len(listReply.Configs) != 1 {
		t.Fatalf("expected list gateway call")
	}
}

func TestValidateDraftUseCaseRejectsInvalidCommand(t *testing.T) {
	t.Parallel()

	_, prob := NewValidateDraftUseCase(&gatewaySpy{}).Execute(context.Background(), contracts.ValidateDraftCommand{})
	if prob == nil {
		t.Fatal("expected problem")
	}
}
