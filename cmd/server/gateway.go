package main

import (
	"context"

	adapternats "internal/adapters/nats"
	configctlcontracts "internal/application/configctl/contracts"
	"internal/application/ports"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/shared/problem"
	"internal/shared/settings"
)

func newConfigctlGateway(config settings.AppConfig) (ports.ConfigctlGateway, func() error, *problem.Problem) {
	if !config.NATS.Enabled {
		return unavailableConfigctlGateway{}, nil, nil
	}

	requestClient, err := adapternats.NewNATSRequestClientWithURL(config.NATS.URL, config.NATS.RequestTimeoutDuration())
	if err != nil {
		return nil, nil, problem.Wrap(err, problem.Unavailable, "failed to initialize configctl request client")
	}

	gateway := adapternats.NewConfigctlGateway(requestClient, "server.http")
	return gateway, requestClient.Close, nil
}

func newValidatorRuntimeGateway(config settings.AppConfig) (ports.ValidatorRuntimeGateway, func() error, *problem.Problem) {
	if !config.NATS.Enabled {
		return unavailableValidatorRuntimeGateway{}, nil, nil
	}

	requestClient, err := adapternats.NewNATSRequestClientWithURL(config.NATS.URL, config.NATS.RequestTimeoutDuration())
	if err != nil {
		return nil, nil, problem.Wrap(err, problem.Unavailable, "failed to initialize validator runtime request client")
	}

	gateway := adapternats.NewValidatorRuntimeGateway(requestClient, "server.http")
	return gateway, requestClient.Close, nil
}

func newValidatorResultsGateway(config settings.AppConfig) (ports.ValidatorResultsGateway, func() error, *problem.Problem) {
	if !config.NATS.Enabled {
		return unavailableValidatorResultsGateway{}, nil, nil
	}

	requestClient, err := adapternats.NewNATSRequestClientWithURL(config.NATS.URL, config.NATS.RequestTimeoutDuration())
	if err != nil {
		return nil, nil, problem.Wrap(err, problem.Unavailable, "failed to initialize validator results request client")
	}

	gateway := adapternats.NewValidatorResultsGateway(requestClient, "server.http")
	return gateway, requestClient.Close, nil
}

type unavailableConfigctlGateway struct{}

func (unavailableConfigctlGateway) CreateDraft(context.Context, configctlcontracts.CreateDraftCommand) (configctlcontracts.CreateDraftReply, *problem.Problem) {
	return configctlcontracts.CreateDraftReply{}, unavailableConfigctlProblem()
}

func (unavailableConfigctlGateway) GetConfig(context.Context, configctlcontracts.GetConfigQuery) (configctlcontracts.GetConfigReply, *problem.Problem) {
	return configctlcontracts.GetConfigReply{}, unavailableConfigctlProblem()
}

func (unavailableConfigctlGateway) GetActiveConfig(context.Context, configctlcontracts.GetActiveConfigQuery) (configctlcontracts.GetActiveConfigReply, *problem.Problem) {
	return configctlcontracts.GetActiveConfigReply{}, unavailableConfigctlProblem()
}

func (unavailableConfigctlGateway) ListActiveRuntimeProjections(context.Context, configctlcontracts.ListActiveRuntimeProjectionsQuery) (configctlcontracts.ListActiveRuntimeProjectionsReply, *problem.Problem) {
	return configctlcontracts.ListActiveRuntimeProjectionsReply{}, unavailableConfigctlProblem()
}

func (unavailableConfigctlGateway) ListActiveIngestionBindings(context.Context, configctlcontracts.ListActiveIngestionBindingsQuery) (configctlcontracts.ListActiveIngestionBindingsReply, *problem.Problem) {
	return configctlcontracts.ListActiveIngestionBindingsReply{}, unavailableConfigctlProblem()
}

func (unavailableConfigctlGateway) ListConfigs(context.Context, configctlcontracts.ListConfigsQuery) (configctlcontracts.ListConfigsReply, *problem.Problem) {
	return configctlcontracts.ListConfigsReply{}, unavailableConfigctlProblem()
}

func (unavailableConfigctlGateway) ValidateDraft(context.Context, configctlcontracts.ValidateDraftCommand) (configctlcontracts.ValidateDraftReply, *problem.Problem) {
	return configctlcontracts.ValidateDraftReply{}, unavailableConfigctlProblem()
}

func (unavailableConfigctlGateway) ValidateConfig(context.Context, configctlcontracts.ValidateConfigCommand) (configctlcontracts.ValidateConfigReply, *problem.Problem) {
	return configctlcontracts.ValidateConfigReply{}, unavailableConfigctlProblem()
}

func (unavailableConfigctlGateway) CompileConfig(context.Context, configctlcontracts.CompileConfigCommand) (configctlcontracts.CompileConfigReply, *problem.Problem) {
	return configctlcontracts.CompileConfigReply{}, unavailableConfigctlProblem()
}

func (unavailableConfigctlGateway) ActivateConfig(context.Context, configctlcontracts.ActivateConfigCommand) (configctlcontracts.ActivateConfigReply, *problem.Problem) {
	return configctlcontracts.ActivateConfigReply{}, unavailableConfigctlProblem()
}

type unavailableValidatorRuntimeGateway struct{}

func (unavailableValidatorRuntimeGateway) GetActiveRuntime(context.Context, runtimecontracts.GetActiveRuntimeQuery) (runtimecontracts.GetActiveRuntimeReply, *problem.Problem) {
	return runtimecontracts.GetActiveRuntimeReply{}, unavailableValidatorRuntimeProblem()
}

type unavailableValidatorResultsGateway struct{}

func (unavailableValidatorResultsGateway) ListValidationResults(context.Context, validatorresultscontracts.ListValidationResultsQuery) (validatorresultscontracts.ListValidationResultsReply, *problem.Problem) {
	return validatorresultscontracts.ListValidationResultsReply{}, unavailableValidatorResultsProblem()
}

func unavailableConfigctlProblem() *problem.Problem {
	return problem.New(problem.Unavailable, "configctl gateway is unavailable because nats is disabled")
}

func unavailableValidatorRuntimeProblem() *problem.Problem {
	return problem.New(problem.Unavailable, "validator runtime gateway is unavailable because nats is disabled")
}

func unavailableValidatorResultsProblem() *problem.Problem {
	return problem.New(problem.Unavailable, "validator results gateway is unavailable because nats is disabled")
}
