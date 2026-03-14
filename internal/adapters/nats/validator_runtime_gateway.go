package nats

import (
	"context"

	"internal/application/ports"
	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/shared/problem"
)

type ValidatorRuntimeGateway struct {
	client   requestReplyClient
	source   string
	registry ValidatorRuntimeRegistry
}

var _ ports.ValidatorRuntimeGateway = (*ValidatorRuntimeGateway)(nil)

func NewValidatorRuntimeGateway(client requestReplyClient, source string) *ValidatorRuntimeGateway {
	return &ValidatorRuntimeGateway{
		client:   client,
		source:   source,
		registry: DefaultValidatorRuntimeRegistry(),
	}
}

func (g *ValidatorRuntimeGateway) GetActiveRuntime(ctx context.Context, query runtimecontracts.GetActiveRuntimeQuery) (runtimecontracts.GetActiveRuntimeReply, *problem.Problem) {
	return invokeRuntimeControl[runtimecontracts.GetActiveRuntimeQuery, runtimecontracts.GetActiveRuntimeReply](ctx, g, g.registry.GetActive, query, "request validator active runtime")
}

func invokeRuntimeControl[Req any, Res any](ctx context.Context, gateway *ValidatorRuntimeGateway, spec ControlSpec, payload Req, action string) (Res, *problem.Problem) {
	var zero Res
	if gateway == nil || gateway.client == nil {
		return zero, problem.New(problem.Unavailable, "validator runtime gateway is unavailable")
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
