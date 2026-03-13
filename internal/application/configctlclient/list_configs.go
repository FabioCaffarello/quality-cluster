package configctlclient

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/shared/problem"
)

type ListConfigsUseCase struct {
	gateway ports.ConfigctlGateway
}

func NewListConfigsUseCase(gateway ports.ConfigctlGateway) *ListConfigsUseCase {
	return &ListConfigsUseCase{gateway: gateway}
}

func (uc *ListConfigsUseCase) Execute(ctx context.Context, query contracts.ListConfigsQuery) (contracts.ListConfigsReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return contracts.ListConfigsReply{}, problem.New(problem.Unavailable, "config service is unavailable")
	}

	return uc.gateway.ListConfigs(ctx, query)
}
