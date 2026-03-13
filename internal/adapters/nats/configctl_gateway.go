package nats

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/shared/problem"
)

type ConfigctlGateway struct {
	client   requestReplyClient
	source   string
	registry ConfigctlRegistry
}

var _ ports.ConfigctlGateway = (*ConfigctlGateway)(nil)

func NewConfigctlGateway(client requestReplyClient, source string) *ConfigctlGateway {
	return &ConfigctlGateway{
		client:   client,
		source:   source,
		registry: DefaultConfigctlRegistry(),
	}
}

func (g *ConfigctlGateway) CreateDraft(ctx context.Context, command contracts.CreateDraftCommand) (contracts.CreateDraftReply, *problem.Problem) {
	return invokeControl[contracts.CreateDraftCommand, contracts.CreateDraftReply](ctx, g, g.registry.CreateDraft, command, "request configctl create draft")
}

func (g *ConfigctlGateway) GetConfig(ctx context.Context, query contracts.GetConfigQuery) (contracts.GetConfigReply, *problem.Problem) {
	return invokeControl[contracts.GetConfigQuery, contracts.GetConfigReply](ctx, g, g.registry.GetConfig, query, "request configctl get config")
}

func (g *ConfigctlGateway) GetActiveConfig(ctx context.Context, query contracts.GetActiveConfigQuery) (contracts.GetActiveConfigReply, *problem.Problem) {
	return invokeControl[contracts.GetActiveConfigQuery, contracts.GetActiveConfigReply](ctx, g, g.registry.GetActive, query, "request configctl active config")
}

func (g *ConfigctlGateway) ListConfigs(ctx context.Context, query contracts.ListConfigsQuery) (contracts.ListConfigsReply, *problem.Problem) {
	return invokeControl[contracts.ListConfigsQuery, contracts.ListConfigsReply](ctx, g, g.registry.ListConfigs, query, "request configctl list configs")
}

func (g *ConfigctlGateway) ValidateDraft(ctx context.Context, command contracts.ValidateDraftCommand) (contracts.ValidateDraftReply, *problem.Problem) {
	return invokeControl[contracts.ValidateDraftCommand, contracts.ValidateDraftReply](ctx, g, g.registry.ValidateDraft, command, "request configctl validate draft")
}

func invokeControl[Req any, Res any](ctx context.Context, gateway *ConfigctlGateway, spec ControlSpec, payload Req, action string) (Res, *problem.Problem) {
	var zero Res
	if gateway == nil || gateway.client == nil {
		return zero, problem.New(problem.Unavailable, "configctl gateway is unavailable")
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
