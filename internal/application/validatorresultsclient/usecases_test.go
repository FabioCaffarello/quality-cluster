package validatorresultsclient

import (
	"context"
	"testing"

	validatorresultscontracts "internal/application/validatorresults/contracts"
	"internal/shared/problem"
)

type resultsGatewaySpy struct {
	query validatorresultscontracts.ListValidationResultsQuery
	reply validatorresultscontracts.ListValidationResultsReply
	prob  *problem.Problem
}

func (s *resultsGatewaySpy) ListValidationResults(_ context.Context, query validatorresultscontracts.ListValidationResultsQuery) (validatorresultscontracts.ListValidationResultsReply, *problem.Problem) {
	s.query = query
	return s.reply, s.prob
}

func TestListValidationResultsUseCaseCallsGateway(t *testing.T) {
	t.Parallel()

	gateway := &resultsGatewaySpy{
		reply: validatorresultscontracts.ListValidationResultsReply{
			Results: []validatorresultscontracts.ValidationResultRecord{{MessageID: "msg-1"}},
		},
	}

	reply, prob := NewListValidationResultsUseCase(gateway).Execute(context.Background(), validatorresultscontracts.ListValidationResultsQuery{
		Limit: 5,
	})
	if prob != nil {
		t.Fatalf("list validation results: %v", prob)
	}
	if len(reply.Results) != 1 || reply.Results[0].MessageID != "msg-1" {
		t.Fatalf("unexpected results reply %+v", reply.Results)
	}
	if gateway.query.ScopeKind != "global" || gateway.query.ScopeKey != "default" || gateway.query.Limit != 5 {
		t.Fatalf("unexpected normalized query %+v", gateway.query)
	}
}
