package configctl

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/shared/problem"
)

type GetActiveConfigUseCase struct {
	repository Repository
}

func NewGetActiveConfigUseCase(repository Repository) *GetActiveConfigUseCase {
	return &GetActiveConfigUseCase{repository: repository}
}

func (uc *GetActiveConfigUseCase) Execute(ctx context.Context, _ contracts.GetActiveConfigQuery) (contracts.GetActiveConfigReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.GetActiveConfigReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	config, prob := uc.repository.GetActive(ctx)
	if prob != nil {
		return contracts.GetActiveConfigReply{}, prob
	}

	return contracts.GetActiveConfigReply{
		Config: recordFromDomain(config),
	}, nil
}
