package validatorruntimeclient

import (
	"context"
	"testing"

	sharedruntime "internal/application/runtimecontracts"
	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/shared/problem"
)

type runtimeGatewaySpy struct {
	query runtimecontracts.GetActiveRuntimeQuery
	reply runtimecontracts.GetActiveRuntimeReply
	prob  *problem.Problem
}

func (s *runtimeGatewaySpy) GetActiveRuntime(_ context.Context, query runtimecontracts.GetActiveRuntimeQuery) (runtimecontracts.GetActiveRuntimeReply, *problem.Problem) {
	s.query = query
	return s.reply, s.prob
}

func TestGetActiveRuntimeUseCaseCallsGateway(t *testing.T) {
	t.Parallel()

	gateway := &runtimeGatewaySpy{
		reply: runtimecontracts.GetActiveRuntimeReply{
			Runtime: runtimecontracts.ActiveRuntimeRecord{
				RuntimeRecord: sharedruntime.RuntimeRecord{
					Config: sharedruntime.ConfigRecord{Key: "core"},
				},
			},
		},
	}

	reply, prob := NewGetActiveRuntimeUseCase(gateway).Execute(context.Background(), runtimecontracts.GetActiveRuntimeQuery{
		ScopeKind: "TENANT",
		ScopeKey:  "br",
	})
	if prob != nil {
		t.Fatalf("get active runtime: %v", prob)
	}
	if reply.Runtime.Config.Key != "core" {
		t.Fatalf("expected runtime config key %q, got %q", "core", reply.Runtime.Config.Key)
	}
	if gateway.query.ScopeKind != "tenant" || gateway.query.ScopeKey != "br" {
		t.Fatalf("unexpected runtime query normalization: %+v", gateway.query)
	}
}
