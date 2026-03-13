package configctl

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/shared/problem"
)

type ListConfigsUseCase struct {
	repository Repository
}

func NewListConfigsUseCase(repository Repository) *ListConfigsUseCase {
	return &ListConfigsUseCase{repository: repository}
}

func (uc *ListConfigsUseCase) Execute(ctx context.Context, _ contracts.ListConfigsQuery) (contracts.ListConfigsReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.ListConfigsReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	configs, prob := uc.repository.List(ctx)
	if prob != nil {
		return contracts.ListConfigsReply{}, prob
	}

	reply := contracts.ListConfigsReply{
		Configs: make([]contracts.ConfigRecord, 0, len(configs)),
	}
	for _, config := range configs {
		reply.Configs = append(reply.Configs, recordFromDomain(config))
	}

	return reply, nil
}
