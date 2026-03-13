package main

import (
	"context"
	adapternats "internal/adapters/nats"
	configctlcontracts "internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/shared/problem"
	"internal/shared/settings"
)


func newConfigctlGateway(config settings.AppConfig) (ports.ConfigctlGateway, func() error, *problem.Problem) {
	if !config.NATS.Enabled {
		return unavailableGateway{}, nil, nil
	}

	requestClient, err := adapternats.NewNATSRequestClientWithURL(config.NATS.URL, config.NATS.RequestTimeoutDuration())
	if err != nil {
		return nil, nil, problem.Wrap(err, problem.Unavailable, "failed to initialize configctl request client")
	}

	gateway := adapternats.NewConfigctlGateway(requestClient, "server.http")
	return gateway, requestClient.Close, nil
}

type unavailableGateway struct{}

func (unavailableGateway) CreateDraft(context.Context, configctlcontracts.CreateDraftCommand) (configctlcontracts.CreateDraftReply, *problem.Problem) {
	return configctlcontracts.CreateDraftReply{}, unavailableProblem()
}

func (unavailableGateway) GetConfig(context.Context, configctlcontracts.GetConfigQuery) (configctlcontracts.GetConfigReply, *problem.Problem) {
	return configctlcontracts.GetConfigReply{}, unavailableProblem()
}

func (unavailableGateway) GetActiveConfig(context.Context, configctlcontracts.GetActiveConfigQuery) (configctlcontracts.GetActiveConfigReply, *problem.Problem) {
	return configctlcontracts.GetActiveConfigReply{}, unavailableProblem()
}

func (unavailableGateway) ListConfigs(context.Context, configctlcontracts.ListConfigsQuery) (configctlcontracts.ListConfigsReply, *problem.Problem) {
	return configctlcontracts.ListConfigsReply{}, unavailableProblem()
}

func (unavailableGateway) ValidateDraft(context.Context, configctlcontracts.ValidateDraftCommand) (configctlcontracts.ValidateDraftReply, *problem.Problem) {
	return configctlcontracts.ValidateDraftReply{}, unavailableProblem()
}

func unavailableProblem() *problem.Problem {
	return problem.New(problem.Unavailable, "configctl gateway is unavailable because nats is disabled")
}
