package configctlclient

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/shared/problem"
)

type ValidateConfigUseCase struct {
	gateway ports.ConfigctlGateway
}

func NewValidateConfigUseCase(gateway ports.ConfigctlGateway) *ValidateConfigUseCase {
	return &ValidateConfigUseCase{gateway: gateway}
}

func (uc *ValidateConfigUseCase) Execute(ctx context.Context, command contracts.ValidateConfigCommand) (contracts.ValidateConfigReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return contracts.ValidateConfigReply{}, problem.New(problem.Unavailable, "config service is unavailable")
	}

	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.ValidateConfigReply{}, prob
	}

	return uc.gateway.ValidateConfig(ctx, command)
}
