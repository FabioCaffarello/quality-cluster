package main

import (
	"context"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/interfaces/http/handlers"
)

func newConfigctlReadinessChecker(gateway ports.ConfigctlGateway) handlers.ReadinessChecker {
	return handlers.ReadinessCheckerFunc(func(ctx context.Context) error {
		if gateway == nil {
			return unavailableProblem()
		}

		_, prob := gateway.ListConfigs(ctx, configctlcontracts.ListConfigsQuery{})
		if prob != nil {
			return prob
		}

		return nil
	})
}
