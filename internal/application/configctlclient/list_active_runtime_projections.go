package configctlclient

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/shared/problem"
)

type ListActiveRuntimeProjectionsUseCase struct {
	gateway ports.ConfigctlGateway
}

func NewListActiveRuntimeProjectionsUseCase(gateway ports.ConfigctlGateway) *ListActiveRuntimeProjectionsUseCase {
	return &ListActiveRuntimeProjectionsUseCase{gateway: gateway}
}

func (uc *ListActiveRuntimeProjectionsUseCase) Execute(ctx context.Context, query contracts.ListActiveRuntimeProjectionsQuery) (contracts.ListActiveRuntimeProjectionsReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return contracts.ListActiveRuntimeProjectionsReply{}, problem.New(problem.Unavailable, "config service is unavailable")
	}

	query = query.Normalize()
	if prob := query.Validate(); prob != nil {
		return contracts.ListActiveRuntimeProjectionsReply{}, prob
	}

	return uc.gateway.ListActiveRuntimeProjections(ctx, query)
}
