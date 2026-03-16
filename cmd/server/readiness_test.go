package main

import (
	"context"
	"testing"

	configctlcontracts "internal/application/configctl/contracts"
	sharedruntime "internal/application/runtimecontracts"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/shared/problem"
	"internal/shared/settings"
)

type readinessConfigctlGatewayStub struct {
	listConfigsReply configctlcontracts.ListConfigsReply
	listConfigsProb  *problem.Problem
	bindingsReply    configctlcontracts.ListActiveIngestionBindingsReply
	bindingsProb     *problem.Problem
	bindingsQuery    configctlcontracts.ListActiveIngestionBindingsQuery
}

func (s *readinessConfigctlGatewayStub) CreateDraft(context.Context, configctlcontracts.CreateDraftCommand) (configctlcontracts.CreateDraftReply, *problem.Problem) {
	return configctlcontracts.CreateDraftReply{}, nil
}

func (s *readinessConfigctlGatewayStub) GetConfig(context.Context, configctlcontracts.GetConfigQuery) (configctlcontracts.GetConfigReply, *problem.Problem) {
	return configctlcontracts.GetConfigReply{}, nil
}

func (s *readinessConfigctlGatewayStub) GetActiveConfig(context.Context, configctlcontracts.GetActiveConfigQuery) (configctlcontracts.GetActiveConfigReply, *problem.Problem) {
	return configctlcontracts.GetActiveConfigReply{}, nil
}

func (s *readinessConfigctlGatewayStub) ListActiveRuntimeProjections(context.Context, configctlcontracts.ListActiveRuntimeProjectionsQuery) (configctlcontracts.ListActiveRuntimeProjectionsReply, *problem.Problem) {
	return configctlcontracts.ListActiveRuntimeProjectionsReply{}, nil
}

func (s *readinessConfigctlGatewayStub) ListActiveIngestionBindings(_ context.Context, query configctlcontracts.ListActiveIngestionBindingsQuery) (configctlcontracts.ListActiveIngestionBindingsReply, *problem.Problem) {
	s.bindingsQuery = query
	return s.bindingsReply, s.bindingsProb
}

func (s *readinessConfigctlGatewayStub) ListConfigs(context.Context, configctlcontracts.ListConfigsQuery) (configctlcontracts.ListConfigsReply, *problem.Problem) {
	return s.listConfigsReply, s.listConfigsProb
}

func (s *readinessConfigctlGatewayStub) ValidateDraft(context.Context, configctlcontracts.ValidateDraftCommand) (configctlcontracts.ValidateDraftReply, *problem.Problem) {
	return configctlcontracts.ValidateDraftReply{}, nil
}

func (s *readinessConfigctlGatewayStub) ValidateConfig(context.Context, configctlcontracts.ValidateConfigCommand) (configctlcontracts.ValidateConfigReply, *problem.Problem) {
	return configctlcontracts.ValidateConfigReply{}, nil
}

func (s *readinessConfigctlGatewayStub) CompileConfig(context.Context, configctlcontracts.CompileConfigCommand) (configctlcontracts.CompileConfigReply, *problem.Problem) {
	return configctlcontracts.CompileConfigReply{}, nil
}

func (s *readinessConfigctlGatewayStub) ActivateConfig(context.Context, configctlcontracts.ActivateConfigCommand) (configctlcontracts.ActivateConfigReply, *problem.Problem) {
	return configctlcontracts.ActivateConfigReply{}, nil
}

type readinessRuntimeGatewayStub struct {
	reply runtimecontracts.GetActiveRuntimeReply
	prob  *problem.Problem
	query runtimecontracts.GetActiveRuntimeQuery
}

func (s *readinessRuntimeGatewayStub) GetActiveRuntime(_ context.Context, query runtimecontracts.GetActiveRuntimeQuery) (runtimecontracts.GetActiveRuntimeReply, *problem.Problem) {
	s.query = query
	return s.reply, s.prob
}

type readinessResultsGatewayStub struct {
	reply validatorresultscontracts.ListValidationResultsReply
	prob  *problem.Problem
	query validatorresultscontracts.ListValidationResultsQuery
}

func (s *readinessResultsGatewayStub) ListValidationResults(_ context.Context, query validatorresultscontracts.ListValidationResultsQuery) (validatorresultscontracts.ListValidationResultsReply, *problem.Problem) {
	s.query = query
	return s.reply, s.prob
}

func TestServerReadinessCheckerUsesCompositeDependencies(t *testing.T) {
	t.Parallel()

	configctl := &readinessConfigctlGatewayStub{}
	runtime := &readinessRuntimeGatewayStub{
		reply: runtimecontracts.GetActiveRuntimeReply{
			Runtime: runtimecontracts.ActiveRuntimeRecord{
				RuntimeRecord: sharedruntime.RuntimeRecord{
					Config: sharedruntime.ConfigRecord{VersionID: "cfg-123"},
				},
			},
		},
	}
	results := &readinessResultsGatewayStub{}

	checker := newServerReadinessChecker(settings.AppConfig{
		NATS: settings.NATSConfig{Enabled: true},
	}, configctl, runtime, results)

	if err := checker.Check(context.Background()); err != nil {
		t.Fatalf("expected readiness to pass, got %v", err)
	}
	if configctl.bindingsQuery.ScopeKind != "global" || configctl.bindingsQuery.ScopeKey != "default" {
		t.Fatalf("unexpected bindings scope query: %+v", configctl.bindingsQuery)
	}
	if runtime.query.ScopeKind != "global" || runtime.query.ScopeKey != "default" {
		t.Fatalf("unexpected runtime scope query: %+v", runtime.query)
	}
	if results.query.ScopeKind != "global" || results.query.ScopeKey != "default" || results.query.Limit != 1 {
		t.Fatalf("unexpected results scope query: %+v", results.query)
	}
}

func TestServerReadinessCheckerAcceptsRuntimeNotFoundWhenNoBindingsAreActive(t *testing.T) {
	t.Parallel()

	checker := newServerReadinessChecker(settings.AppConfig{
		NATS: settings.NATSConfig{Enabled: true},
	}, &readinessConfigctlGatewayStub{}, &readinessRuntimeGatewayStub{
		prob: problem.New(problem.NotFound, "validator runtime is not loaded"),
	}, &readinessResultsGatewayStub{})

	if err := checker.Check(context.Background()); err != nil {
		t.Fatalf("expected readiness to pass without active bindings, got %v", err)
	}
}

func TestServerReadinessCheckerFailsWhenBindingsExistWithoutRuntime(t *testing.T) {
	t.Parallel()

	checker := newServerReadinessChecker(settings.AppConfig{
		NATS: settings.NATSConfig{Enabled: true},
	}, &readinessConfigctlGatewayStub{
		bindingsReply: configctlcontracts.ListActiveIngestionBindingsReply{
			Bindings: []configctlcontracts.ActiveIngestionBindingRecord{{Binding: configctlcontracts.BindingRecord{Name: "orders"}}},
		},
	}, &readinessRuntimeGatewayStub{
		prob: problem.New(problem.NotFound, "validator runtime is not loaded"),
	}, &readinessResultsGatewayStub{})

	if err := checker.Check(context.Background()); err == nil {
		t.Fatal("expected readiness to fail when bindings exist without runtime")
	}
}

func TestServerReadinessCheckerFailsWhenNATSIsDisabled(t *testing.T) {
	t.Parallel()

	checker := newServerReadinessChecker(settings.AppConfig{}, &readinessConfigctlGatewayStub{}, &readinessRuntimeGatewayStub{}, &readinessResultsGatewayStub{})
	if err := checker.Check(context.Background()); err == nil {
		t.Fatal("expected readiness to fail when nats is disabled")
	}
}
