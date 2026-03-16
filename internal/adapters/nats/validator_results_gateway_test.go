package nats

import (
	"context"
	"testing"

	validatorresultscontracts "internal/application/validatorresults/contracts"
)

func TestValidatorResultsGatewayListValidationResults(t *testing.T) {
	t.Parallel()

	registry := DefaultValidatorResultsRegistry()
	request := mustDecodeRequest[validatorresultscontracts.ListValidationResultsQuery](t, registry.List, mustEncodeRequest(t, registry.List, validatorresultscontracts.ListValidationResultsQuery{
		Limit:  5,
		Status: validatorresultscontracts.ValidationStatusFailed,
	}))
	replyBytes, err := encodeControlReply(
		registry.List,
		"validator",
		request,
		validatorresultscontracts.ListValidationResultsReply{
			Results: []validatorresultscontracts.ValidationResultRecord{{MessageID: "msg-1"}},
		},
		nil,
	)
	if err != nil {
		t.Fatalf("encode results reply: %v", err)
	}

	client := &requestReplyClientSpy{reply: replyBytes}
	reply, prob := NewValidatorResultsGateway(client, "server.http").ListValidationResults(context.Background(), validatorresultscontracts.ListValidationResultsQuery{
		Limit:  5,
		Status: validatorresultscontracts.ValidationStatusFailed,
	})
	if prob != nil {
		t.Fatalf("list validation results: %v", prob)
	}
	if client.subject != registry.List.Subject {
		t.Fatalf("expected subject %q, got %q", registry.List.Subject, client.subject)
	}
	if len(reply.Results) != 1 || reply.Results[0].MessageID != "msg-1" {
		t.Fatalf("unexpected results reply %+v", reply.Results)
	}
}
