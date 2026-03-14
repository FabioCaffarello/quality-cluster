package configctlclient

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/shared/problem"
)

type CompileConfigUseCase struct {
	gateway ports.ConfigctlGateway
}

func NewCompileConfigUseCase(gateway ports.ConfigctlGateway) *CompileConfigUseCase {
	return &CompileConfigUseCase{gateway: gateway}
}

func (uc *CompileConfigUseCase) Execute(ctx context.Context, command contracts.CompileConfigCommand) (contracts.CompileConfigReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return contracts.CompileConfigReply{}, problem.New(problem.Unavailable, "config service is unavailable")
	}

	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.CompileConfigReply{}, prob
	}

	return uc.gateway.CompileConfig(ctx, command)
}
