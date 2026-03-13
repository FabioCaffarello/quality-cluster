package configctlclient

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/shared/problem"
)

type GetActiveConfigUseCase struct {
	gateway ports.ConfigctlGateway
}

func NewGetActiveConfigUseCase(gateway ports.ConfigctlGateway) *GetActiveConfigUseCase {
	return &GetActiveConfigUseCase{gateway: gateway}
}

func (uc *GetActiveConfigUseCase) Execute(ctx context.Context, query contracts.GetActiveConfigQuery) (contracts.GetActiveConfigReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return contracts.GetActiveConfigReply{}, problem.New(problem.Unavailable, "config service is unavailable")
	}

	return uc.gateway.GetActiveConfig(ctx, query)
}
