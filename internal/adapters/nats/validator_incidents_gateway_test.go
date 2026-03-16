package nats

import (
	"context"
	"testing"

	validatorincidentscontracts "internal/application/validatorincidents/contracts"
)

func TestValidatorIncidentsGatewayListValidationIncidents(t *testing.T) {
	t.Parallel()

	registry := DefaultValidatorIncidentsRegistry()
	request := mustDecodeRequest[validatorincidentscontracts.ListValidationIncidentsQuery](t, registry.List, mustEncodeRequest(t, registry.List, validatorincidentscontracts.ListValidationIncidentsQuery{
		Limit:  5,
		Status: validatorincidentscontracts.ValidationIncidentStatusOpen,
	}))
	replyBytes, err := encodeControlReply(
		registry.List,
		"validator",
		request,
		validatorincidentscontracts.ListValidationIncidentsReply{
			Incidents: []validatorincidentscontracts.ValidationIncidentRecord{{IncidentKey: "incident-1"}},
		},
		nil,
	)
	if err != nil {
		t.Fatalf("encode incidents reply: %v", err)
	}

	client := &requestReplyClientSpy{reply: replyBytes}
	reply, prob := NewValidatorIncidentsGateway(client, "server.http").ListValidationIncidents(context.Background(), validatorincidentscontracts.ListValidationIncidentsQuery{
		Limit:  5,
		Status: validatorincidentscontracts.ValidationIncidentStatusOpen,
	})
	if prob != nil {
		t.Fatalf("list validation incidents: %v", prob)
	}
	if client.subject != registry.List.Subject {
		t.Fatalf("expected subject %q, got %q", registry.List.Subject, client.subject)
	}
	if len(reply.Incidents) != 1 || reply.Incidents[0].IncidentKey != "incident-1" {
		t.Fatalf("unexpected incidents reply %+v", reply.Incidents)
	}
}
