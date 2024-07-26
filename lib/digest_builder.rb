# frozen_string_literal: true

class DigestBuilder
  A_DAY = 24 * 60 * 60 # Seconds in a day.
  private_constant :A_DAY

  def initialize(storage_adapter:)
    @storage = storage_adapter
  end

  def build_digest(digest_strategy:, date:, posts:)
    yesterday_digest = @storage.fetch_digest(
      type: digest_strategy.type,
      date: date - A_DAY
    )

    unsent_posts = remove_sent_posts(
      all_posts: posts,
      yesterday_digest:
    ).sort_by { |post| post['points'] }.reverse

    selected_posts = digest_strategy.select(unsent_posts)

    @storage.save_digest(
      type: digest_strategy.type,
      date:,
      posts: selected_posts
    )

    selected_posts
  end

  def remove_sent_posts(all_posts:, yesterday_digest:)
    return all_posts if yesterday_digest.nil?

    yesterday_posts = yesterday_digest['posts']
    return all_posts if yesterday_posts.nil?

    sent_post_ids = yesterday_posts.to_set { |post| post['objectID'] }
    all_posts.reject { |post| sent_post_ids.include?(post['objectID']) }
  end
end
