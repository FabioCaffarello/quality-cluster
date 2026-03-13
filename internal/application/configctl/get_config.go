package configctl

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/shared/problem"
)

type GetConfigUseCase struct {
	repository Repository
}

func NewGetConfigUseCase(repository Repository) *GetConfigUseCase {
	return &GetConfigUseCase{repository: repository}
}

func (uc *GetConfigUseCase) Execute(ctx context.Context, query contracts.GetConfigQuery) (contracts.GetConfigReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.GetConfigReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	query = query.Normalize()
	if prob := query.Validate(); prob != nil {
		return contracts.GetConfigReply{}, prob
	}

	config, prob := uc.repository.GetByID(ctx, query.ID)
	if prob != nil {
		return contracts.GetConfigReply{}, prob
	}

	return contracts.GetConfigReply{
		Config: recordFromDomain(config),
	}, nil
}
