package configctl

import (
	"context"

	"internal/application/configctl/contracts"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"
)

type GetActiveConfigUseCase struct {
	repository Repository
}

func NewGetActiveConfigUseCase(repository Repository) *GetActiveConfigUseCase {
	return &GetActiveConfigUseCase{repository: repository}
}

func (uc *GetActiveConfigUseCase) Execute(ctx context.Context, query contracts.GetActiveConfigQuery) (contracts.GetActiveConfigReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.GetActiveConfigReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	query = query.Normalize()
	activation, prob := uc.repository.GetActivationByScope(ctx, configdomain.ActivationScope{
		Kind: query.ScopeKind,
		Key:  query.ScopeKey,
	}.Normalize())
	if prob != nil {
		return contracts.GetActiveConfigReply{}, prob
	}
	set, prob := uc.repository.GetConfigSetByVersionID(ctx, activation.VersionID)
	if prob != nil {
		return contracts.GetActiveConfigReply{}, prob
	}
	version, ok := set.VersionByID(activation.VersionID)
	if !ok {
		return contracts.GetActiveConfigReply{}, problem.New(problem.NotFound, "config version not found")
	}
	activations, prob := uc.repository.ListActivationsByVersionID(ctx, activation.VersionID)
	if prob != nil {
		return contracts.GetActiveConfigReply{}, prob
	}

	return contracts.GetActiveConfigReply{
		Config: detailRecordFromDomain(set, version, activations),
	}, nil
}
