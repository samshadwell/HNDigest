# frozen_string_literal: true

module Strategies
  class TopNPosts
    def initialize(num_posts)
      @n = num_posts
    end

    def type
      "TOP_N##{@n}"
    end

    def select(all_posts)
      sorted = all_posts.sort_by { |post| post['points'] }.reverse
      sorted.first(@n)
    end
  end
end
