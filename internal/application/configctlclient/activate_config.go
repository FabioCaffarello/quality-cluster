package configctlclient

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/shared/problem"
)

type ActivateConfigUseCase struct {
	gateway ports.ConfigctlGateway
}

func NewActivateConfigUseCase(gateway ports.ConfigctlGateway) *ActivateConfigUseCase {
	return &ActivateConfigUseCase{gateway: gateway}
}

func (uc *ActivateConfigUseCase) Execute(ctx context.Context, command contracts.ActivateConfigCommand) (contracts.ActivateConfigReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return contracts.ActivateConfigReply{}, problem.New(problem.Unavailable, "config service is unavailable")
	}

	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.ActivateConfigReply{}, prob
	}

	return uc.gateway.ActivateConfig(ctx, command)
}
