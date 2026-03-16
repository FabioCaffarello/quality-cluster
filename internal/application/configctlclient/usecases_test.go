package configctlclient

import (
	"context"
	"testing"

	"internal/application/configctl/contracts"
	"internal/shared/problem"
)

type gatewaySpy struct {
	createDraftCommand         contracts.CreateDraftCommand
	getConfigQuery             contracts.GetConfigQuery
	listCalled                 bool
	listRuntimeProjectionsCall bool
	listIngestionBindingsQuery contracts.ListActiveIngestionBindingsQuery
	validateCommand            contracts.ValidateDraftCommand
	validateConfigCmd          contracts.ValidateConfigCommand
	compileCommand             contracts.CompileConfigCommand
	activateCommand            contracts.ActivateConfigCommand

	createDraftReply            contracts.CreateDraftReply
	getConfigReply              contracts.GetConfigReply
	listReply                   contracts.ListConfigsReply
	listRuntimeProjectionsReply contracts.ListActiveRuntimeProjectionsReply
	validateReply               contracts.ValidateDraftReply
	validateConfigReply         contracts.ValidateConfigReply
	compileReply                contracts.CompileConfigReply
	activateReply               contracts.ActivateConfigReply
	prob                        *problem.Problem
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

func (s *gatewaySpy) ListActiveRuntimeProjections(context.Context, contracts.ListActiveRuntimeProjectionsQuery) (contracts.ListActiveRuntimeProjectionsReply, *problem.Problem) {
	s.listRuntimeProjectionsCall = true
	return s.listRuntimeProjectionsReply, s.prob
}

func (s *gatewaySpy) ListActiveIngestionBindings(_ context.Context, query contracts.ListActiveIngestionBindingsQuery) (contracts.ListActiveIngestionBindingsReply, *problem.Problem) {
	s.listIngestionBindingsQuery = query
	return contracts.ListActiveIngestionBindingsReply{
		Bindings: []contracts.ActiveIngestionBindingRecord{{Binding: contracts.BindingRecord{Name: "orders"}}},
	}, s.prob
}

func (s *gatewaySpy) ListConfigs(context.Context, contracts.ListConfigsQuery) (contracts.ListConfigsReply, *problem.Problem) {
	s.listCalled = true
	return s.listReply, s.prob
}

func (s *gatewaySpy) ValidateDraft(_ context.Context, command contracts.ValidateDraftCommand) (contracts.ValidateDraftReply, *problem.Problem) {
	s.validateCommand = command
	return s.validateReply, s.prob
}

func (s *gatewaySpy) ValidateConfig(_ context.Context, command contracts.ValidateConfigCommand) (contracts.ValidateConfigReply, *problem.Problem) {
	s.validateConfigCmd = command
	return s.validateConfigReply, s.prob
}

func (s *gatewaySpy) CompileConfig(_ context.Context, command contracts.CompileConfigCommand) (contracts.CompileConfigReply, *problem.Problem) {
	s.compileCommand = command
	return s.compileReply, s.prob
}

func (s *gatewaySpy) ActivateConfig(_ context.Context, command contracts.ActivateConfigCommand) (contracts.ActivateConfigReply, *problem.Problem) {
	s.activateCommand = command
	return s.activateReply, s.prob
}

func TestCreateDraftUseCaseCallsGateway(t *testing.T) {
	t.Parallel()

	gateway := &gatewaySpy{
		createDraftReply: contracts.CreateDraftReply{
			Config: contracts.ConfigVersionDetail{ID: "cfg-123"},
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
			Config: contracts.ConfigVersionDetail{ID: "cfg-123"},
		},
		listReply: contracts.ListConfigsReply{
			Configs: []contracts.ConfigVersionSummary{{ID: "cfg-123"}},
		},
	}

	getReply, prob := NewGetConfigUseCase(gateway).Execute(context.Background(), contracts.GetConfigQuery{VersionID: "cfg-123"})
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

	ingestionReply, prob := NewListActiveIngestionBindingsUseCase(gateway).Execute(context.Background(), contracts.ListActiveIngestionBindingsQuery{
		ScopeKind: "tenant",
		ScopeKey:  "br",
	})
	if prob != nil {
		t.Fatalf("list ingestion bindings: %v", prob)
	}
	if gateway.listIngestionBindingsQuery.ScopeKind != "tenant" || gateway.listIngestionBindingsQuery.ScopeKey != "br" {
		t.Fatalf("unexpected ingestion bindings query: %+v", gateway.listIngestionBindingsQuery)
	}
	if len(ingestionReply.Bindings) != 1 || ingestionReply.Bindings[0].Binding.Name != "orders" {
		t.Fatalf("unexpected ingestion bindings reply: %+v", ingestionReply.Bindings)
	}

	runtimeReply, prob := NewListActiveRuntimeProjectionsUseCase(&gatewaySpy{
		listRuntimeProjectionsReply: contracts.ListActiveRuntimeProjectionsReply{
			Runtimes: []contracts.RuntimeProjectionRecord{{VersionID: "cfg-123"}},
		},
	}).Execute(context.Background(), contracts.ListActiveRuntimeProjectionsQuery{})
	if prob != nil {
		t.Fatalf("list runtime projections: %v", prob)
	}
	if len(runtimeReply.Runtimes) != 1 || runtimeReply.Runtimes[0].VersionID != "cfg-123" {
		t.Fatalf("unexpected runtime projections reply: %+v", runtimeReply.Runtimes)
	}
}

func TestValidateDraftUseCaseRejectsInvalidCommand(t *testing.T) {
	t.Parallel()

	_, prob := NewValidateDraftUseCase(&gatewaySpy{}).Execute(context.Background(), contracts.ValidateDraftCommand{})
	if prob == nil {
		t.Fatal("expected problem")
	}
}

func TestLifecycleClientUseCasesCallGateway(t *testing.T) {
	t.Parallel()

	gateway := &gatewaySpy{
		validateConfigReply: contracts.ValidateConfigReply{Valid: true},
		compileReply: contracts.CompileConfigReply{
			Config: contracts.ConfigVersionDetail{ID: "cfg-123", Lifecycle: "compiled"},
		},
		activateReply: contracts.ActivateConfigReply{
			Config: contracts.ConfigVersionDetail{ID: "cfg-123", Lifecycle: "active"},
		},
	}

	if _, prob := NewValidateConfigUseCase(gateway).Execute(context.Background(), contracts.ValidateConfigCommand{VersionID: "cfg-123"}); prob != nil {
		t.Fatalf("validate config: %v", prob)
	}
	if gateway.validateConfigCmd.VersionID != "cfg-123" {
		t.Fatalf("expected validate config id %q, got %q", "cfg-123", gateway.validateConfigCmd.VersionID)
	}

	if _, prob := NewCompileConfigUseCase(gateway).Execute(context.Background(), contracts.CompileConfigCommand{VersionID: "cfg-123"}); prob != nil {
		t.Fatalf("compile config: %v", prob)
	}
	if gateway.compileCommand.VersionID != "cfg-123" {
		t.Fatalf("expected compile config id %q, got %q", "cfg-123", gateway.compileCommand.VersionID)
	}

	if _, prob := NewActivateConfigUseCase(gateway).Execute(context.Background(), contracts.ActivateConfigCommand{VersionID: "cfg-123"}); prob != nil {
		t.Fatalf("activate config: %v", prob)
	}
	if gateway.activateCommand.VersionID != "cfg-123" {
		t.Fatalf("expected activate config id %q, got %q", "cfg-123", gateway.activateCommand.VersionID)
	}
}
