# frozen_string_literal: true

require_relative '../lib/post_fetcher'
require_relative '../lib/storage_adapter'

class SnapshotPostsJob
  # TODO: Where to put these?
  TOP_K_VALUES = [10, 20, 50].freeze
  POINT_THRESHOLD_VALUES = [500, 250, 100].freeze

  LOOKBACK = 2 * 24 * 60 * 60 # 2 days in seconds.
  private_constant :LOOKBACK

  def run(time:)
    # 2x top K in case all the top k were sent yesterday.
    posts = PostFetcher.fetch(top_k: 2 * TOP_K_VALUES.max,
                              points: POINT_THRESHOLD_VALUES.min,
                              since: time - LOOKBACK)

    storage = StorageAdapter.new
    storage.snapshot_posts(posts: posts, time: time)
  end
end
