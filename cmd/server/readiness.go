package main

import (
	"context"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/application/ports"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/interfaces/http/handlers"
	"internal/shared/problem"
	"internal/shared/settings"
)

func newServerReadinessChecker(config settings.AppConfig, configctl ports.ConfigctlGateway, runtime ports.ValidatorRuntimeGateway, results ports.ValidatorResultsGateway) handlers.ReadinessChecker {
	return handlers.ReadinessCheckerFunc(func(ctx context.Context) error {
		if !config.NATS.Enabled {
			return problem.New(problem.Unavailable, "server readiness requires nats to be enabled")
		}
		if configctl == nil {
			return unavailableConfigctlProblem()
		}
		if runtime == nil {
			return unavailableValidatorRuntimeProblem()
		}
		if results == nil {
			return unavailableValidatorResultsProblem()
		}

		if _, prob := configctl.ListConfigs(ctx, configctlcontracts.ListConfigsQuery{}); prob != nil {
			return prob
		}
		bindingsReply, prob := configctl.ListActiveIngestionBindings(ctx, configctlcontracts.ListActiveIngestionBindingsQuery{
			ScopeKind: "global",
			ScopeKey:  "default",
		})
		if prob != nil {
			return prob
		}
		if _, prob := results.ListValidationResults(ctx, validatorresultscontracts.ListValidationResultsQuery{
			ScopeKind: "global",
			ScopeKey:  "default",
			Limit:     1,
		}); prob != nil {
			return prob
		}

		runtimeReply, runtimeProb := runtime.GetActiveRuntime(ctx, runtimecontracts.GetActiveRuntimeQuery{
			ScopeKind: "global",
			ScopeKey:  "default",
		})
		switch {
		case runtimeProb == nil:
		case runtimeProb.Code == problem.NotFound:
			if len(bindingsReply.Bindings) > 0 {
				return problem.New(problem.Unavailable, "validator runtime is stale: active ingestion bindings exist without active validator runtime")
			}
		default:
			return runtimeProb
		}

		if len(bindingsReply.Bindings) > 0 && runtimeReply.Runtime.Config.VersionID == "" {
			return problem.New(problem.Unavailable, "validator runtime is stale: active ingestion bindings exist without loaded validator runtime")
		}

		return nil
	})
}
