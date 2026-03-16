package validatorincidentsclient

import (
	"context"
	"testing"

	validatorincidentscontracts "internal/application/validatorincidents/contracts"
	"internal/shared/problem"
)

type incidentsGatewaySpy struct {
	query validatorincidentscontracts.ListValidationIncidentsQuery
	reply validatorincidentscontracts.ListValidationIncidentsReply
	prob  *problem.Problem
}

func (s *incidentsGatewaySpy) ListValidationIncidents(_ context.Context, query validatorincidentscontracts.ListValidationIncidentsQuery) (validatorincidentscontracts.ListValidationIncidentsReply, *problem.Problem) {
	s.query = query
	return s.reply, s.prob
}

func TestListValidationIncidentsUseCaseCallsGateway(t *testing.T) {
	t.Parallel()

	gateway := &incidentsGatewaySpy{
		reply: validatorincidentscontracts.ListValidationIncidentsReply{
			Incidents: []validatorincidentscontracts.ValidationIncidentRecord{{IncidentKey: "incident-1"}},
		},
	}

	reply, prob := NewListValidationIncidentsUseCase(gateway).Execute(context.Background(), validatorincidentscontracts.ListValidationIncidentsQuery{
		Limit:  5,
		Status: validatorincidentscontracts.ValidationIncidentStatusOpen,
	})
	if prob != nil {
		t.Fatalf("list validation incidents: %v", prob)
	}
	if len(reply.Incidents) != 1 || reply.Incidents[0].IncidentKey != "incident-1" {
		t.Fatalf("unexpected incidents reply %+v", reply.Incidents)
	}
	if gateway.query.ScopeKind != "global" || gateway.query.ScopeKey != "default" || gateway.query.Status != validatorincidentscontracts.ValidationIncidentStatusOpen || gateway.query.Limit != 5 {
		t.Fatalf("unexpected normalized query %+v", gateway.query)
	}
}
