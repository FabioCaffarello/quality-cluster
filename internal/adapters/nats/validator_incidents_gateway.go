package nats

import (
	"context"

	"internal/application/ports"
	validatorincidentscontracts "internal/application/validatorincidents/contracts"
	"internal/shared/problem"
)

type ValidatorIncidentsGateway struct {
	client   requestReplyClient
	source   string
	registry ValidatorIncidentsRegistry
}

var _ ports.ValidatorIncidentsGateway = (*ValidatorIncidentsGateway)(nil)

func NewValidatorIncidentsGateway(client requestReplyClient, source string) *ValidatorIncidentsGateway {
	return &ValidatorIncidentsGateway{
		client:   client,
		source:   source,
		registry: DefaultValidatorIncidentsRegistry(),
	}
}

func (g *ValidatorIncidentsGateway) ListValidationIncidents(ctx context.Context, query validatorincidentscontracts.ListValidationIncidentsQuery) (validatorincidentscontracts.ListValidationIncidentsReply, *problem.Problem) {
	return invokeValidatorIncidentsControl[validatorincidentscontracts.ListValidationIncidentsQuery, validatorincidentscontracts.ListValidationIncidentsReply](ctx, g, g.registry.List, query, "request validator validation incidents")
}

func invokeValidatorIncidentsControl[Req any, Res any](ctx context.Context, gateway *ValidatorIncidentsGateway, spec ControlSpec, payload Req, action string) (Res, *problem.Problem) {
	var zero Res
	if gateway == nil || gateway.client == nil {
		return zero, problem.New(problem.Unavailable, "validator incidents gateway is unavailable")
	}

	requestBytes, prob := encodeControlRequest(ctx, spec, gateway.source, payload)
	if prob != nil {
		return zero, prob
	}

	replyBytes, err := gateway.client.Request(ctx, spec.Subject, requestBytes)
	if err != nil {
		return zero, problem.Wrap(err, problem.Unavailable, action+" failed")
	}

	return decodeControlReply[Res](spec, replyBytes)
}
