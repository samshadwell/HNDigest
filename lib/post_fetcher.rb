# frozen_string_literal: true

require 'http'
require 'json'

class PostFetcher
  HOST = 'https://hn.algolia.com'
  PATH = '/api/v1/search'
  SECONDS_IN_DAY = 60 * 60 * 24

  def self.fetch(top_k:, points:, since:)
    HTTP.persistent(HOST) do |client|
      top_k = fetch_top_k(top_k, client: client, since: since)
      by_points = fetch_by_points(points, client: client, since: since)

      top_k.merge(by_points)
    end
  end

  def self.fetch_top_k(top_k, client:, since:)
    path = PATH + "?hitsPerPage=#{top_k}&" \
      'tags=story&' \
      "numericFilters=created_at_i>=#{since.to_i}"

    fetch_posts_from_path(path, client: client)
  end
  private_class_method :fetch_top_k

  def self.fetch_by_points(points, client:, since:)
    path = PATH + '?hitsPerPage=10000&' \
      'tags=story&' \
      "numericFilters=created_at_i>=#{since.to_i},points>=#{points}"

    fetch_posts_from_path(path, client: client)
  end
  private_class_method :fetch_by_points

  def self.fetch_posts_from_path(path, client:)
    result = JSON.parse(client.get(path).to_s, symbolize_names: true)
    posts = result[:hits].map do |full_p|
      full_p.slice(:created_at, :title, :url, :points, :objectID)
    end

    posts.map { |p| [p[:objectID], p] }.to_h
  end
  private_class_method :fetch_posts_from_path
end
