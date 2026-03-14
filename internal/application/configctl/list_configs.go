package configctl

import (
	"context"
	"sort"

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

	configSets, prob := uc.repository.ListConfigSets(ctx)
	if prob != nil {
		return contracts.ListConfigsReply{}, prob
	}

	type listedVersion struct {
		record contracts.ConfigVersionSummary
	}
	records := make([]listedVersion, 0)
	for _, set := range configSets {
		for _, version := range set.Versions {
			activations, activationsProb := uc.repository.ListActivationsByVersionID(ctx, version.ID)
			if activationsProb != nil {
				return contracts.ListConfigsReply{}, activationsProb
			}
			records = append(records, listedVersion{
				record: summaryRecordFromDomain(set, version, activations),
			})
		}
	}
	sort.SliceStable(records, func(i, j int) bool {
		if records[i].record.CreatedAt.Equal(records[j].record.CreatedAt) {
			return records[i].record.ID > records[j].record.ID
		}
		return records[i].record.CreatedAt.After(records[j].record.CreatedAt)
	})

	reply := contracts.ListConfigsReply{
		Configs: make([]contracts.ConfigVersionSummary, 0, len(records)),
	}
	for _, item := range records {
		reply.Configs = append(reply.Configs, item.record)
	}

	return reply, nil
}
