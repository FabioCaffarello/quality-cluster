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

	set, prob := uc.repository.GetConfigSetByVersionID(ctx, query.VersionID)
	if prob != nil {
		return contracts.GetConfigReply{}, prob
	}
	version, ok := set.VersionByID(query.VersionID)
	if !ok {
		return contracts.GetConfigReply{}, problem.New(problem.NotFound, "config version not found")
	}
	activations, prob := uc.repository.ListActivationsByVersionID(ctx, query.VersionID)
	if prob != nil {
		return contracts.GetConfigReply{}, prob
	}

	return contracts.GetConfigReply{
		Config: detailRecordFromDomain(set, version, activations),
	}, nil
}
