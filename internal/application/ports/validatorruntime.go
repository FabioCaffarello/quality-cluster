package ports

import (
	"context"

	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/shared/problem"
)

type ValidatorRuntimeGateway interface {
	GetActiveRuntime(context.Context, runtimecontracts.GetActiveRuntimeQuery) (runtimecontracts.GetActiveRuntimeReply, *problem.Problem)
}
