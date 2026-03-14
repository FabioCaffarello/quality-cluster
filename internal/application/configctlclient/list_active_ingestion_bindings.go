package configctlclient

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/shared/problem"
)

type ListActiveIngestionBindingsUseCase struct {
	gateway ports.ConfigctlGateway
}

func NewListActiveIngestionBindingsUseCase(gateway ports.ConfigctlGateway) *ListActiveIngestionBindingsUseCase {
	return &ListActiveIngestionBindingsUseCase{gateway: gateway}
}

func (uc *ListActiveIngestionBindingsUseCase) Execute(ctx context.Context, query contracts.ListActiveIngestionBindingsQuery) (contracts.ListActiveIngestionBindingsReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return contracts.ListActiveIngestionBindingsReply{}, problem.New(problem.Unavailable, "config service is unavailable")
	}

	return uc.gateway.ListActiveIngestionBindings(ctx, query)
}
