package configctlclient

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/shared/problem"
)

type GetConfigUseCase struct {
	gateway ports.ConfigctlGateway
}

func NewGetConfigUseCase(gateway ports.ConfigctlGateway) *GetConfigUseCase {
	return &GetConfigUseCase{gateway: gateway}
}

func (uc *GetConfigUseCase) Execute(ctx context.Context, query contracts.GetConfigQuery) (contracts.GetConfigReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return contracts.GetConfigReply{}, problem.New(problem.Unavailable, "config service is unavailable")
	}

	query = query.Normalize()
	if prob := query.Validate(); prob != nil {
		return contracts.GetConfigReply{}, prob
	}

	return uc.gateway.GetConfig(ctx, query)
}
