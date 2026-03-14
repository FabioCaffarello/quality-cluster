package nats

import (
	"context"

	"internal/application/ports"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	"internal/shared/problem"
)

type ValidatorResultsGateway struct {
	client   requestReplyClient
	source   string
	registry ValidatorResultsRegistry
}

var _ ports.ValidatorResultsGateway = (*ValidatorResultsGateway)(nil)

func NewValidatorResultsGateway(client requestReplyClient, source string) *ValidatorResultsGateway {
	return &ValidatorResultsGateway{
		client:   client,
		source:   source,
		registry: DefaultValidatorResultsRegistry(),
	}
}

func (g *ValidatorResultsGateway) ListValidationResults(ctx context.Context, query validatorresultscontracts.ListValidationResultsQuery) (validatorresultscontracts.ListValidationResultsReply, *problem.Problem) {
	return invokeValidatorResultsControl[validatorresultscontracts.ListValidationResultsQuery, validatorresultscontracts.ListValidationResultsReply](ctx, g, g.registry.List, query, "request validator validation results")
}

func invokeValidatorResultsControl[Req any, Res any](ctx context.Context, gateway *ValidatorResultsGateway, spec ControlSpec, payload Req, action string) (Res, *problem.Problem) {
	var zero Res
	if gateway == nil || gateway.client == nil {
		return zero, problem.New(problem.Unavailable, "validator results gateway is unavailable")
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
