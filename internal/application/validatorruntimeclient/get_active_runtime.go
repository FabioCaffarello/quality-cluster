package validatorruntimeclient

import (
	"context"

	"internal/application/ports"
	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/shared/problem"
)

type GetActiveRuntimeUseCase struct {
	gateway ports.ValidatorRuntimeGateway
}

func NewGetActiveRuntimeUseCase(gateway ports.ValidatorRuntimeGateway) *GetActiveRuntimeUseCase {
	return &GetActiveRuntimeUseCase{gateway: gateway}
}

func (uc *GetActiveRuntimeUseCase) Execute(ctx context.Context, query runtimecontracts.GetActiveRuntimeQuery) (runtimecontracts.GetActiveRuntimeReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return runtimecontracts.GetActiveRuntimeReply{}, problem.New(problem.Unavailable, "validator runtime service is unavailable")
	}

	return uc.gateway.GetActiveRuntime(ctx, query.Normalize())
}
